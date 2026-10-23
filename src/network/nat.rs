// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{ConfiguredVeth, FirewallCloseFailure, FirewallTable};
use netlink_packet_core::{DefaultNla, NLM_F_APPEND, NLM_F_CREATE, NLM_F_EXCL};
use netlink_packet_netfilter::nftables::*;
use std::{io, time::Instant};

/// A scoped inet policy: outbound masquerade, replies toward the command, and
/// no unsolicited inbound. Forwarding readiness is a separate environment gate.
/// The connection borrow prevents explicit interface teardown before policy close.
#[derive(Debug)]
pub struct NatPolicy<'a> {
    table: FirewallTable,
    connection: &'a ConfiguredVeth,
}
#[derive(Debug, thiserror::Error)]
#[error("NAT teardown: {failure}")]
pub struct NatCloseFailure<'a> {
    failure: FirewallCloseFailure,
    connection: &'a ConfiguredVeth,
}
impl NatCloseFailure<'_> {
    pub fn cleanup(self) -> Result<(), Self> {
        self.failure.cleanup().map_err(|failure| Self {
            failure,
            connection: self.connection,
        })
    }
}
impl<'a> NatPolicy<'a> {
    pub fn install(connection: &'a ConfiguredVeth, deadline: Instant) -> io::Result<Self> {
        let interface = connection.host_name().as_ref();
        let name = format!("conglobate_{}", interface.replace('-', "_"));
        let plan = NatRules {
            table: name.clone(),
            interface: interface.into(),
        };
        let table = FirewallTable::install(&name, plan.messages(), deadline)?;
        Ok(Self { table, connection })
    }
    pub fn close(self, deadline: Instant) -> Result<(), NatCloseFailure<'a>> {
        self.table
            .close(deadline)
            .map_err(|failure| NatCloseFailure {
                failure,
                connection: self.connection,
            })
    }
}
#[derive(Debug)]
struct NatRules {
    table: String,
    interface: String,
}
impl NatRules {
    fn compare(value: Vec<u8>, op: Operator) -> Expressions {
        Expressions::Cmp(vec![
            Cmp::SourceRegister(Register::Reg1),
            Cmp::Op(op),
            Cmp::Data(DataAttribute::Value(value)),
        ])
    }
    fn meta(key: MetaKey) -> Expressions {
        Expressions::Meta(vec![
            Meta::Key(key),
            Meta::DestinationRegister(Register::Reg1),
        ])
    }
    fn interface(&self, key: MetaKey) -> Vec<Expressions> {
        let mut bytes = self.interface.as_bytes().to_vec();
        bytes.push(0);
        vec![Self::meta(key), Self::compare(bytes, Operator::Equal)]
    }
    fn verdict(value: u32) -> Expressions {
        Expressions::Immediate(vec![
            Immediate::DestinationRegister(Register::Verdict),
            Immediate::Data(DataAttribute::Verdict(vec![VerdictAttribute::Code(
                Verdict::Other(value),
            )])),
        ])
    }
    fn rule(&self, chain: &str, expressions: Vec<Expressions>) -> (NfTablesMessage, u16) {
        (
            NfTablesMessage::NewRule(RuleMessage {
                attributes: vec![
                    RuleAttribute::Table(self.table.clone()),
                    RuleAttribute::Chain(chain.into()),
                    RuleAttribute::Expressions(expressions.into_iter().map(Into::into).collect()),
                ],
            }),
            NLM_F_CREATE | NLM_F_APPEND,
        )
    }
    fn chain(
        &self,
        name: &str,
        hook: InetHookNumber,
        kind: &str,
        priority: u32,
    ) -> (NfTablesMessage, u16) {
        (
            NfTablesMessage::NewChain(ChainMessage {
                attributes: vec![
                    ChainAttribute::Table(self.table.clone()),
                    ChainAttribute::Name(name.into()),
                    ChainAttribute::Type(kind.into()),
                    ChainAttribute::Hook(vec![
                        Hook::Number(HookNumber::Inet(hook)),
                        Hook::Priority(priority),
                    ]),
                    ChainAttribute::Policy(1),
                ],
            }),
            NLM_F_CREATE | NLM_F_EXCL,
        )
    }
    fn messages(&self) -> Vec<(NfTablesMessage, u16)> {
        let mut requests = vec![
            self.chain("forward", InetHookNumber::Forward, "filter", 0),
            self.chain("output", InetHookNumber::LocalOut, "filter", 0),
            self.chain("nat", InetHookNumber::PostRouting, "nat", 100),
        ];
        for chain in ["forward", "output"] {
            let mut replies = self.interface(MetaKey::Oifname);
            // Linux UAPI NFTA_CT_DREG=1, NFTA_CT_KEY=2, NFT_CT_STATE=0.
            // Safe library generic attributes cover ct/masq not yet typed by it.
            replies.push(Expressions::Other {
                expression_type: "ct".into(),
                attributes: vec![
                    DefaultNla::new(1, 1u32.to_be_bytes().to_vec()),
                    DefaultNla::new(2, 0u32.to_be_bytes().to_vec()),
                ],
            });
            // NF_CT_STATE_BIT(ESTABLISHED)|NF_CT_STATE_BIT(RELATED) = 2|4.
            replies.push(Expressions::Bitwise(vec![
                Bitwise::SourceRegister(Register::Reg1),
                Bitwise::DestinationRegister(Register::Reg1),
                Bitwise::Length(4),
                Bitwise::Mask(DataAttribute::Value(6u32.to_ne_bytes().to_vec())),
                Bitwise::Xor(DataAttribute::Value(vec![0; 4])),
            ]));
            replies.push(Self::compare(vec![0; 4], Operator::NotEqual));
            replies.push(Self::verdict(1));
            requests.push(self.rule(chain, replies));
            if chain == "output" {
                // Locally originated IPv6 neighbor discovery is necessary for
                // the private link. It is not forwarded inbound application data.
                for kind in [135, 136] {
                    let mut nd = self.interface(MetaKey::Oifname);
                    nd.extend([
                        Self::meta(MetaKey::Nfproto),
                        Self::compare(vec![10], Operator::Equal),
                        Self::meta(MetaKey::L4Proto),
                        Self::compare(vec![58], Operator::Equal),
                    ]);
                    nd.push(Expressions::Payload(vec![
                        Payload::DestinationRegister(Register::Reg1),
                        Payload::Base(1),
                        Payload::Offset(7),
                        Payload::Len(1),
                    ]));
                    nd.push(Self::compare(vec![255], Operator::Equal));
                    nd.push(Expressions::Payload(vec![
                        Payload::DestinationRegister(Register::Reg1),
                        Payload::Base(2),
                        Payload::Offset(0),
                        Payload::Len(1),
                    ]));
                    nd.push(Self::compare(vec![kind], Operator::Equal));
                    nd.push(Self::verdict(1));
                    requests.push(self.rule(chain, nd));
                }
            }
            let mut drop = self.interface(MetaKey::Oifname);
            drop.push(Self::verdict(0));
            requests.push(self.rule(chain, drop));
        }
        let mut nat = self.interface(MetaKey::Iifname);
        nat.push(Expressions::Other {
            expression_type: "masq".into(),
            attributes: vec![],
        });
        requests.push(self.rule("nat", nat));
        requests
    }
}

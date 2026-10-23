#!/bin/bash
set -eux
modprobe veth
modprobe nf_tables
modprobe nft_chain_nat
modprobe nft_masq
modprobe nft_ct
/lifecycle-tests network-composite

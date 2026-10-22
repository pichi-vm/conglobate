// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
#[error("COPY mode must be unsigned octal permissions or symbolic permission clauses")]
pub struct ModeError;

#[derive(Debug)]
pub enum Permissions {
    Octal(u32),
    Symbolic(Vec<Clause>),
}
#[derive(Debug)]
pub struct Clause {
    who: u32,
    changes: Vec<Change>,
}
#[derive(Debug)]
pub struct Change {
    operator: Operator,
    permissions: Vec<Permission>,
}
#[derive(Debug)]
pub enum Operator {
    Add,
    Remove,
    Assign,
}
#[derive(Debug)]
pub enum Permission {
    Read,
    Write,
    Execute,
    ConditionalExecute,
    SetId,
    Sticky,
    User,
    Group,
    Other,
}

#[derive(Debug)]
pub struct CopyMode(Permissions);
impl FromStr for CopyMode {
    type Err = ModeError;
    fn from_str(source: &str) -> Result<Self, Self::Err> {
        if !source.is_empty() && source.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
            let mode = u32::from_str_radix(source, 8).map_err(|_| ModeError)?;
            return if mode <= 0o7777 {
                Ok(Self(Permissions::Octal(mode)))
            } else {
                Err(ModeError)
            };
        }
        let clauses = source
            .split(',')
            .map(Clause::from_str)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self(Permissions::Symbolic(clauses)))
    }
}
impl FromStr for Clause {
    type Err = ModeError;
    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let mut chars = source.chars().peekable();
        let mut who = 0;
        while let Some(bits) = chars.peek().and_then(|ch| match ch {
            'u' => Some(0o4700),
            'g' => Some(0o2070),
            'o' => Some(0o7),
            'a' => Some(0o6777),
            _ => None,
        }) {
            who |= bits;
            chars.next();
        }
        let mut changes = Vec::new();
        while let Some(ch) = chars.next() {
            let operator = match ch {
                '+' => Operator::Add,
                '-' => Operator::Remove,
                '=' => Operator::Assign,
                _ => return Err(ModeError),
            };
            let mut permissions = Vec::new();
            while chars
                .peek()
                .is_some_and(|ch| !matches!(ch, '+' | '-' | '='))
            {
                permissions.push(match chars.next() {
                    Some('r') => Permission::Read,
                    Some('w') => Permission::Write,
                    Some('x') => Permission::Execute,
                    Some('X') => Permission::ConditionalExecute,
                    Some('s') => Permission::SetId,
                    Some('t') => Permission::Sticky,
                    Some('u') => Permission::User,
                    Some('g') => Permission::Group,
                    Some('o') => Permission::Other,
                    _ => return Err(ModeError),
                });
            }
            changes.push(Change {
                operator,
                permissions,
            });
        }
        if changes.is_empty() {
            return Err(ModeError);
        }
        Ok(Self { who, changes })
    }
}
impl CopyMode {
    pub fn permissions(&self) -> &Permissions {
        &self.0
    }
}
impl Clause {
    pub fn who(&self) -> u32 {
        self.who
    }
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }
}
impl Change {
    pub fn operator(&self) -> &Operator {
        &self.operator
    }
    pub fn permissions(&self) -> &[Permission] {
        &self.permissions
    }
}

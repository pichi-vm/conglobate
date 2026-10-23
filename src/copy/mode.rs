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

impl CopyMode {
    /// Apply parsed COPY permissions with BuildKit's zero-umask semantics.
    pub fn apply(&self, original: u32, directory: bool) -> u32 {
        match &self.0 {
            Permissions::Octal(mode) => *mode,
            Permissions::Symbolic(clauses) => {
                let mut mode = original & 0o7777;
                for clause in clauses {
                    for change in &clause.changes {
                        mode = change.apply(mode, clause.who, directory || original & 0o111 != 0);
                    }
                }
                mode
            }
        }
    }
}
impl Operator {
    fn apply(&self, mode: u32, bits: u32, affected: u32) -> u32 {
        match self {
            Self::Add => mode | (bits & affected),
            Self::Remove => mode & !(bits & affected),
            Self::Assign => (mode & !affected) | (bits & affected),
        }
    }
}
impl Change {
    fn apply(&self, mut mode: u32, mut who: u32, executable: bool) -> u32 {
        let mut bits = 0;
        let mut conditional = false;
        let mut assigned = false;
        for permission in &self.permissions {
            match permission {
                Permission::Read => bits |= 0o444,
                Permission::Write => bits |= 0o222,
                Permission::Execute => bits |= 0o111,
                Permission::ConditionalExecute => {
                    if matches!(self.operator, Operator::Remove) {
                        bits |= 0o111;
                    } else {
                        conditional = true;
                    }
                }
                Permission::SetId => bits |= 0o6000,
                Permission::Sticky => {
                    if who == 0 || who & !0o7 != 0 {
                        who |= 0o1000;
                        bits |= 0o1000;
                    }
                }
                Permission::User | Permission::Group | Permission::Other => {
                    let affected = if who == 0 { 0o6777 } else { who };
                    if bits != 0 {
                        mode = self.operator.apply(mode, bits, affected);
                        bits = 0;
                    }
                    if conditional && executable {
                        mode |= 0o111 & affected;
                    }
                    conditional = false;
                    let shift = match permission {
                        Permission::User => 6,
                        Permission::Group => 3,
                        _ => 0,
                    };
                    let value = ((mode >> shift) & 7) * 0o111;
                    // Copy operations affect ordinary permission bits only.
                    mode = self.operator.apply(mode, value, affected & 0o777);
                    assigned |= matches!(self.operator, Operator::Assign);
                }
            }
        }
        let affected = if who == 0 { 0o6777 } else { who };
        if bits != 0 || (matches!(self.operator, Operator::Assign) && !assigned) {
            mode = self.operator.apply(mode, bits, affected);
        }
        if conditional && executable {
            mode |= 0o111 & affected;
        }
        mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_buildkit_reference_for_all_permission_bits() {
        use sha2::{Digest, Sha256};
        #[derive(serde::Deserialize)]
        struct Case {
            mode: String,
            directory: bool,
            digest: String,
        }
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../../tests/data/copy-mode-reference.json"))
                .unwrap();
        for case in cases {
            let mode: CopyMode = case.mode.parse().unwrap();
            let mut hash = Sha256::new();
            for initial in 0..4096 {
                hash.update(mode.apply(initial, case.directory).to_be_bytes());
            }
            assert_eq!(
                hex::encode(hash.finalize()),
                case.digest,
                "{} directory={}",
                case.mode,
                case.directory
            );
        }
    }
    #[test]
    fn permission_application_handles_original_execute_and_sequential_copy() {
        for (text, initial, directory, expected) in [
            ("0755", 0o1644, false, 0o755),
            ("a+X", 0o640, false, 0o640),
            ("a+X", 0o640, true, 0o751),
            ("a-x,a+X", 0o751, false, 0o751),
            ("u+x,g+X", 0o640, false, 0o740),
            ("a=X", 0o751, false, 0o111),
            ("a=X", 0o640, false, 0),
            ("g=u", 0o640, false, 0o660),
            ("u=rw,go=r", 0o777, false, 0o644),
            ("g=uw", 0o740, false, 0o720),
            ("g=wu", 0o740, false, 0o770),
            ("u=go", 0o764, false, 0o464),
            ("u=g", 0o4640, false, 0o4440),
            ("+rt", 0, false, 0o1000),
            ("a+rt", 0, false, 0o1444),
            ("o+t", 0o644, false, 0o644),
            ("u+t=rw", 0o644, false, 0o1644),
            ("a=rw-t", 0o1644, false, 0o666),
            ("=rw", 0o1644, false, 0o1666),
        ] {
            assert_eq!(
                text.parse::<CopyMode>().unwrap().apply(initial, directory),
                expected,
                "{text}"
            );
        }
    }
}

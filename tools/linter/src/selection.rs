// SPDX-License-Identifier: AGPL-3.0-only
// SPDX-FileCopyrightText: 2026 Torrust project contributors

//! Generic owner-scoped selection between an execution plan and a content policy.
//!
//! The two public run types deliberately keep constitutive inclusion and a
//! diagnostic gloss apart. A constitutive run admits a path only when exactly
//! one inclusion row reaches it. A gloss run admits every path its exclusions
//! leave and then judges an optional inclusion partition without letting that
//! judgment move the governed set.

use std::collections::BTreeMap;

use crate::declaration::AbnfPattern;
use crate::pattern::BytePath;

/// One named pattern in a policy section.
#[derive(Debug, Clone)]
pub struct Rule<P> {
    name: String,
    pattern: AbnfPattern,
    payload: P,
}

impl<P> Rule<P> {
    pub const fn new(name: String, pattern: AbnfPattern, payload: P) -> Self {
        Self {
            name,
            pattern,
            payload,
        }
    }

    fn display(&self) -> String {
        format!("{} : {}", self.name, self.pattern.source())
    }
}

/// One owner-and-context section whose inclusion rows constitute selection.
#[derive(Debug, Clone)]
pub struct ConstitutiveSection<C, P> {
    owner: String,
    context: C,
    exclude: Vec<Rule<()>>,
    include: Vec<Rule<P>>,
}

impl<C, P> ConstitutiveSection<C, P> {
    pub const fn new(
        owner: String,
        context: C,
        exclude: Vec<Rule<()>>,
        include: Vec<Rule<P>>,
    ) -> Self {
        Self {
            owner,
            context,
            exclude,
            include,
        }
    }
}

/// One owner section whose optional inclusion rows are diagnostic gloss.
#[derive(Debug, Clone)]
pub struct GlossSection {
    owner: String,
    exclude: Vec<Rule<()>>,
    gloss: Option<Vec<Rule<()>>>,
}

impl GlossSection {
    pub const fn new(owner: String, exclude: Vec<Rule<()>>, gloss: Option<Vec<Rule<()>>>) -> Self {
        Self {
            owner,
            exclude,
            gloss,
        }
    }
}

/// Which list an idle row belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum List {
    Exclude,
    Include,
}

/// One constitutively selected path and the payload its unique row carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstitutiveEntry<C, P> {
    pub path: BytePath,
    pub owner: String,
    pub context: C,
    pub payload: P,
}

/// One path selected independently of its optional gloss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlossEntry {
    pub path: BytePath,
    pub owner: String,
    pub row: Option<String>,
}

/// One named exclusion match retained for audit explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Excluded<C> {
    pub path: BytePath,
    pub owner: String,
    pub context: C,
    pub name: String,
}

/// A defect in a constitutive inclusion judgment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstitutiveDefect<C> {
    Uncovered {
        path: BytePath,
        owner: String,
        context: C,
    },
    MultiplyIncluded {
        path: BytePath,
        owner: String,
        context: C,
        matches: Vec<String>,
    },
    IdleRow {
        owner: String,
        context: C,
        list: List,
        name: String,
    },
}

/// A defect in an optional diagnostic gloss.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlossDefect {
    Uncovered {
        path: BytePath,
        owner: String,
    },
    MultiplyIncluded {
        path: BytePath,
        owner: String,
        matches: Vec<String>,
    },
    IdleRow {
        owner: String,
        list: List,
        name: String,
    },
}

/// The output of constitutive owner/exclude/include selection.
#[derive(Debug, Clone)]
pub struct ConstitutiveRun<C, P> {
    pub governed: Vec<ConstitutiveEntry<C, P>>,
    pub excluded: Vec<Excluded<C>>,
    pub defects: Vec<ConstitutiveDefect<C>>,
}

/// The output of owner/exclude selection followed by diagnostic gloss.
#[derive(Debug, Clone)]
pub struct GlossRun {
    pub governed: Vec<GlossEntry>,
    pub excluded: Vec<Excluded<()>>,
    pub defects: Vec<GlossDefect>,
}

/// Select through constitutive inclusion rows.
pub fn constitutive<C: Clone, P: Clone>(
    attribution: &BTreeMap<&BytePath, &str>,
    sections: &[ConstitutiveSection<C, P>],
) -> ConstitutiveRun<C, P> {
    let mut governed = Vec::new();
    let mut excluded = Vec::new();
    let mut defects = Vec::new();

    for section in sections {
        let mut reached: BTreeMap<(List, &str), bool> = BTreeMap::new();

        for row in &section.exclude {
            reached.insert((List::Exclude, row.name.as_str()), false);
        }
        for row in &section.include {
            reached.insert((List::Include, row.name.as_str()), false);
        }

        for (path, owner) in attribution {
            if *owner != section.owner {
                continue;
            }

            for row in &section.exclude {
                if row.pattern.admits_path(path) {
                    reached.insert((List::Exclude, row.name.as_str()), true);
                    excluded.push(Excluded {
                        path: (*path).clone(),
                        owner: section.owner.clone(),
                        context: section.context.clone(),
                        name: row.name.clone(),
                    });
                }
            }
            for row in &section.include {
                if row.pattern.admits_path(path) {
                    reached.insert((List::Include, row.name.as_str()), true);
                }
            }
        }

        for (path, owner) in attribution {
            if *owner != section.owner
                || section
                    .exclude
                    .iter()
                    .any(|row| row.pattern.admits_path(path))
            {
                continue;
            }

            let matched: Vec<_> = section
                .include
                .iter()
                .filter(|row| row.pattern.admits_path(path))
                .collect();

            match matched.as_slice() {
                [row] => governed.push(ConstitutiveEntry {
                    path: (*path).clone(),
                    owner: section.owner.clone(),
                    context: section.context.clone(),
                    payload: row.payload.clone(),
                }),
                [] => defects.push(ConstitutiveDefect::Uncovered {
                    path: (*path).clone(),
                    owner: section.owner.clone(),
                    context: section.context.clone(),
                }),
                rows => {
                    let mut matching_rows: Vec<_> = rows.iter().map(|row| row.display()).collect();
                    matching_rows.sort();
                    defects.push(ConstitutiveDefect::MultiplyIncluded {
                        path: (*path).clone(),
                        owner: section.owner.clone(),
                        context: section.context.clone(),
                        matches: matching_rows,
                    });
                }
            }
        }

        for ((list, name), found) in reached {
            if !found {
                defects.push(ConstitutiveDefect::IdleRow {
                    owner: section.owner.clone(),
                    context: section.context.clone(),
                    list,
                    name: name.to_owned(),
                });
            }
        }
    }

    ConstitutiveRun {
        governed,
        excluded,
        defects,
    }
}

/// Select by exclusion and then judge an optional diagnostic gloss.
pub fn diagnostic_gloss(
    attribution: &BTreeMap<&BytePath, &str>,
    sections: &[GlossSection],
) -> GlossRun {
    let mut governed = Vec::new();
    let mut excluded = Vec::new();
    let mut defects = Vec::new();

    for section in sections {
        let mut reached: BTreeMap<(List, &str), bool> = BTreeMap::new();

        for row in &section.exclude {
            reached.insert((List::Exclude, row.name.as_str()), false);
        }
        if let Some(gloss) = &section.gloss {
            for row in gloss {
                reached.insert((List::Include, row.name.as_str()), false);
            }
        }

        for (path, owner) in attribution {
            if *owner != section.owner {
                continue;
            }

            let mut removed = false;
            for row in &section.exclude {
                if row.pattern.admits_path(path) {
                    reached.insert((List::Exclude, row.name.as_str()), true);
                    excluded.push(Excluded {
                        path: (*path).clone(),
                        owner: section.owner.clone(),
                        context: (),
                        name: row.name.clone(),
                    });
                    removed = true;
                }
            }
            if removed {
                continue;
            }

            let matched: Vec<_> = section
                .gloss
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter(|row| row.pattern.admits_path(path))
                .collect();

            for row in &matched {
                reached.insert((List::Include, row.name.as_str()), true);
            }

            let row = match (&section.gloss, matched.as_slice()) {
                (None, _) => None,
                (Some(_), [row]) => Some(row.name.clone()),
                (Some(_), []) => {
                    defects.push(GlossDefect::Uncovered {
                        path: (*path).clone(),
                        owner: section.owner.clone(),
                    });
                    None
                }
                (Some(_), rows) => {
                    let mut matching_rows: Vec<_> = rows.iter().map(|row| row.display()).collect();
                    matching_rows.sort();
                    defects.push(GlossDefect::MultiplyIncluded {
                        path: (*path).clone(),
                        owner: section.owner.clone(),
                        matches: matching_rows,
                    });
                    None
                }
            };

            governed.push(GlossEntry {
                path: (*path).clone(),
                owner: section.owner.clone(),
                row,
            });
        }

        for ((list, name), found) in reached {
            if !found {
                defects.push(GlossDefect::IdleRow {
                    owner: section.owner.clone(),
                    list,
                    name: name.to_owned(),
                });
            }
        }
    }

    GlossRun {
        governed,
        excluded,
        defects,
    }
}

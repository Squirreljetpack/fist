use std::ffi::OsString;

use indexmap::IndexMap;

use crate::abspath::AbsPath;

pub enum ExclusiveList {
    Vec(Vec<(AbsPath, OsString)>),
    Map(IndexMap<AbsPath, OsString>),
}

impl ExclusiveList {
    pub fn push(
        &mut self,
        path: AbsPath,
        dst: OsString,
    ) {
        match self {
            ExclusiveList::Vec(v) => v.push((path, dst)),
            ExclusiveList::Map(m) => {
                m.insert(path, dst);
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ExclusiveList::Vec(v) => v.len(),
            ExclusiveList::Map(m) => m.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(
        &self,
        index: usize,
    ) -> Option<(AbsPath, OsString)> {
        match self {
            ExclusiveList::Vec(v) => v.get(index).cloned(),
            ExclusiveList::Map(m) => m.get_index(index).map(|(p, d)| (p.clone(), d.clone())),
        }
    }

    // no retain because no progress
    pub fn clear(&mut self) {
        match self {
            ExclusiveList::Vec(v) => v.clear(),
            ExclusiveList::Map(m) => m.clear(),
        }
    }

    pub fn update(
        &mut self,
        index: usize,
        new_path: Option<AbsPath>,
        new_dst: Option<OsString>,
    ) {
        match self {
            ExclusiveList::Vec(v) => {
                if let Some((p, d)) = v.get_mut(index) {
                    if let Some(p_new) = new_path {
                        *p = p_new;
                    }
                    if let Some(d_new) = new_dst {
                        *d = d_new;
                    }
                }
            }
            ExclusiveList::Map(m) => {
                if let Some((mut p, mut d)) =
                    m.get_index(index).map(|(p, d)| (p.clone(), d.clone()))
                {
                    if let Some(p_new) = new_path {
                        p = p_new;
                    }
                    if let Some(d_new) = new_dst {
                        d = d_new;
                    }
                    m.shift_remove_index(index);
                    m.shift_insert(index, p, d);
                }
            }
        }
    }

    pub fn remove(
        &mut self,
        index: usize,
    ) {
        match self {
            ExclusiveList::Vec(v) => {
                if index < v.len() {
                    v.remove(index);
                }
            }
            ExclusiveList::Map(m) => {
                if index < m.len() {
                    m.shift_remove_index(index);
                }
            }
        }
    }

    pub fn swap(
        &mut self,
        i: usize,
        j: usize,
    ) {
        match self {
            ExclusiveList::Vec(v) => v.swap(i, j),
            ExclusiveList::Map(m) => m.swap_indices(i, j),
        }
    }

    pub fn as_slice(&self) -> Vec<(AbsPath, OsString)> {
        match self {
            ExclusiveList::Vec(v) => v.clone(),
            ExclusiveList::Map(m) => m.iter().map(|(p, d)| (p.clone(), d.clone())).collect(),
        }
    }

    pub fn iter_any(
        &self,
        path: &AbsPath,
    ) -> bool {
        match self {
            ExclusiveList::Vec(v) => v.iter().any(|(p, _)| p == path),
            ExclusiveList::Map(m) => m.contains_key(path),
        }
    }

    pub fn position(
        &self,
        path: &AbsPath,
    ) -> Option<usize> {
        match self {
            ExclusiveList::Vec(v) => v.iter().position(|(p, _)| p == path),
            ExclusiveList::Map(m) => m.get_index_of(path),
        }
    }
}

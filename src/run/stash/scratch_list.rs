use std::ffi::OsString;

use indexmap::IndexMap;

use crate::abspath::AbsPath;

pub enum ScratchList {
    Vec(Vec<(AbsPath, OsString)>, u8),
    Map(IndexMap<AbsPath, OsString>),
}

impl ScratchList {
    pub fn push(
        &mut self,
        path: AbsPath,
        dst: OsString,
    ) {
        match self {
            ScratchList::Vec(v, limit) => {
                let limit = *limit as usize;
                if limit > 0 && v.len() >= limit {
                    v[limit - 1] = (path, dst);
                } else {
                    v.push((path, dst));
                }
            }
            ScratchList::Map(m) => {
                // True rule: if exists, do not add
                if !m.contains_key(&path) {
                    m.insert(path, dst);
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ScratchList::Vec(v, _) => v.len(),
            ScratchList::Map(m) => m.len(),
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
            ScratchList::Vec(v, _) => v.get(index).cloned(),
            ScratchList::Map(m) => m.get_index(index).map(|(p, d)| (p.clone(), d.clone())),
        }
    }

    pub fn clear(&mut self) {
        match self {
            ScratchList::Vec(v, _) => v.clear(),
            ScratchList::Map(m) => m.clear(),
        }
    }

    pub fn update(
        &mut self,
        index: usize,
        new_path: Option<AbsPath>,
        new_dst: Option<OsString>,
    ) {
        match self {
            ScratchList::Vec(v, _) => {
                if let Some((p, d)) = v.get_mut(index) {
                    if let Some(p_new) = new_path {
                        *p = p_new;
                    }
                    if let Some(d_new) = new_dst {
                        *d = d_new;
                    }
                }
            }
            ScratchList::Map(m) => {
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
            ScratchList::Vec(v, _) => {
                if index < v.len() {
                    v.remove(index);
                }
            }
            ScratchList::Map(m) => {
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
            ScratchList::Vec(v, _) => v.swap(i, j),
            ScratchList::Map(m) => m.swap_indices(i, j),
        }
    }

    pub fn as_slice(&self) -> Vec<(AbsPath, OsString)> {
        match self {
            ScratchList::Vec(v, _) => v.clone(),
            ScratchList::Map(m) => m.iter().map(|(p, d)| (p.clone(), d.clone())).collect(),
        }
    }

    pub fn iter_any(
        &self,
        path: &AbsPath,
    ) -> bool {
        match self {
            ScratchList::Vec(v, _) => v.iter().any(|(p, _)| p == path),
            ScratchList::Map(m) => m.contains_key(path),
        }
    }

    pub fn position(
        &self,
        path: &AbsPath,
    ) -> Option<usize> {
        match self {
            ScratchList::Vec(v, _) => v.iter().position(|(p, _)| p == path),
            ScratchList::Map(m) => m.get_index_of(path),
        }
    }
}

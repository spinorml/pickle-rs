//
// Copyright (C) 2023 SpinorML.
//
// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at

//   http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use std::cmp::{Eq, PartialEq};
use std::hash::Hash;
use std::{collections::HashMap, collections::HashSet};

use crate::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct F64Wrapper(pub f64);

impl std::cmp::Eq for F64Wrapper {}

impl std::hash::Hash for F64Wrapper {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let bits = self.0.to_bits();
        bits.hash(state);
    }
}

#[derive(Clone, Debug)]
pub struct HashMapWrapper<K: Eq + Hash, V: Eq + Hash>(pub HashMap<K, V>);

impl<K: Eq + Hash, V: Eq + Hash> HashMapWrapper<K, V> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl<K: Eq + Hash, V: Eq + Hash> Default for HashMapWrapper<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Hash, V: Eq + Hash> std::cmp::PartialEq for HashMapWrapper<K, V>
where
    K: std::cmp::PartialEq,
    V: std::cmp::PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().all(|(k, v)| other.0.get(k) == Some(v))
    }
}
impl<K: Eq + Hash, V: Eq + Hash> std::cmp::Eq for HashMapWrapper<K, V> where K: std::cmp::Eq {}

impl<K: Eq + Hash, V: Eq + std::hash::Hash> std::hash::Hash for HashMapWrapper<K, V>
where
    K: std::hash::Hash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for (k, v) in self.0.iter() {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl From<Vec<(Value, Value)>> for HashMapWrapper<Value, Value> {
    fn from(hm: Vec<(Value, Value)>) -> Self {
        Self(hm.into_iter().collect())
    }
}

#[derive(Clone, Debug)]
pub struct HashSetWrapper<T: Eq + Hash>(pub HashSet<T>);

impl HashSetWrapper<Value> {
    pub fn new() -> Self {
        Self(HashSet::new())
    }
}

impl Default for HashSetWrapper<Value> {
    fn default() -> Self {
        Self::new()
    }
}

impl std::cmp::PartialEq for HashSetWrapper<Value> {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len() && self.0.iter().all(|v| other.0.get(v) == Some(v))
    }
}

impl std::cmp::Eq for HashSetWrapper<Value> {}

impl std::hash::Hash for HashSetWrapper<Value> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for v in self.0.iter() {
            v.hash(state);
        }
    }
}

impl From<Vec<Value>> for HashSetWrapper<Value> {
    fn from(hm: Vec<Value>) -> Self {
        Self(hm.into_iter().collect())
    }
}

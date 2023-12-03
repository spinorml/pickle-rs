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

use std::hash::Hash;

use num_bigint::BigInt;

use crate::{F64Wrapper, HashMapWrapper, HashSetWrapper};

pub type MemoId = u32;

#[derive(Clone, Debug, PartialEq, Hash)]
pub enum Global {
    Set,       // builtins/__builtin__.set
    Frozenset, // builtins/__builtin__.frozenset
    Bytearray, // builtins/__builtin__.bytearray
    List,      // builtins/__builtin__.list
    Int,       // builtins/__builtin__.int
    Encode,    // _codecs.encode
    Other,     // anything else (may be a classobj that is later discarded)
}

#[derive(Clone, Debug, PartialEq, Hash)]
pub enum Value {
    MemoRef(MemoId),
    Global(Global),
    None,
    Bool(bool),
    Int(BigInt),
    I64(i64),
    F64(F64Wrapper),
    Bytes(Vec<u8>),
    String(String),
    List(Vec<Value>),
    Tuple(Vec<Value>),
    Set(HashSetWrapper<Value>),
    FrozenSet(HashSetWrapper<Value>),
    Dict(HashMapWrapper<Value, Value>),
    PersId(String),
    BinPersId(Box<Value>),
}

impl std::cmp::Eq for Value {}

impl From<i128> for Value {
    fn from(i: i128) -> Self {
        Value::Int(BigInt::from(i))
    }
}

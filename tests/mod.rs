//
// Copyright (C) 2023 SpinorML.
// Copyright (c) 2015-2021 Georg Brandl.
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

use std::fs::File;

use num_bigint::BigInt;
use pickle_rs::{Error, ErrorCode, HashableValue, Unpickler, UnpicklerOptions, Value};

macro_rules! pyobj {
    (n=None)     => { Value::None };
    (b=True)     => { Value::Bool(true) };
    (b=False)    => { Value::Bool(false) };
    (i=$i:expr)  => { Value::I64($i) };
    (ii=$i:expr) => { Value::Int($i.clone()) };
    (f=$f:expr)  => { Value::F64($f) };
    (bb=$b:expr) => { Value::Bytes($b.to_vec()) };
    (s=$s:expr)  => { Value::String($s.into()) };
    (t=($($m:ident=$v:tt),*))  => { Value::Tuple(vec![$(pyobj!($m=$v)),*]) };
    (l=[$($m:ident=$v:tt),*])  => { Value::List(vec![$(pyobj!($m=$v)),*]) };
    (ss=($($m:ident=$v:tt),*)) => { Value::Set(vec![$(hpyobj!($m=$v)),*]) };
    (fs=($($m:ident=$v:tt),*)) => { Value::FrozenSet(vec![$(hpyobj!($m=$v)),*]) };
    (d={$($km:ident=$kv:tt => $vm:ident=$vv:tt),*}) => {
        Value::Dict(vec![$((hpyobj!($km=$kv), pyobj!($vm=$vv))),*])
    };
}

macro_rules! hpyobj {
    (n=None)     => { HashableValue::None };
    (b=True)     => { HashableValue::Bool(true) };
    (b=False)    => { HashableValue::Bool(false) };
    (i=$i:expr)  => { HashableValue::I64($i) };
    (ii=$i:expr) => { HashableValue::Int($i.clone()) };
    (f=$f:expr)  => { HashableValue::F64($f) };
    (bb=$b:expr) => { HashableValue::Bytes($b.to_vec()) };
    (s=$s:expr)  => { HashableValue::String($s.into()) };
    (t=($($m:ident=$v:tt),*))  => { HashableValue::Tuple(vec![$(hpyobj!($m=$v)),*]) };
    (fs=($($m:ident=$v:tt),*)) => { HashableValue::FrozenSet(vec![$(hpyobj!($m=$v)),*]) };
}

// combinations of (python major, pickle proto) to test
const TEST_CASES: &[(u32, u32)] = &[
    (2, 0),
    (2, 1),
    (2, 2),
    (3, 0),
    (3, 1),
    (3, 2),
    (3, 3),
    (3, 4),
    (3, 5),
];

fn get_test_object(pyver: u32) -> Value {
    let longish = BigInt::from(10000000000u64) * BigInt::from(10000000000u64);
    let mut obj = pyobj!(d={
        n=None           => n=None,
        b=False          => t=(b=False, b=True),
        i=10             => i=100000,
        ii=longish       => ii=longish,
        f=1.0            => f=1.0,
        bb=b"bytes"      => bb=b"bytes",
        s="string"       => s="string",
        fs=(i=0, i=42)   => fs=(i=0, i=42),
        t=(i=1, i=2)     => t=(i=1, i=2, i=3),
        t=()             => l=[
            l=[i=1, i=2, i=3],
            ss=(i=0, i=42),
            d={},
            bb=b"\x00\x55\xaa\xff"
        ]
    });
    // Unfortunately, __dict__ keys are strings and so are pickled
    // differently depending on major version.
    match &mut obj {
        Value::Dict(map) => {
            if pyver == 2 {
                map.push((hpyobj!(i = 7), pyobj!(d={bb=b"attr" => i=5})));
            } else {
                map.push((hpyobj!(i = 7), pyobj!(d={s="attr" => i=5})));
            }
        }
        _ => unreachable!(),
    }
    obj
}

#[test]
fn unpickle_all() {
    for &(major, proto) in TEST_CASES {
        let filename = format!("tests/data/tests_py{}_proto{}.pickle", major, proto);
        let file = File::open(filename).unwrap();
        let unpickler = Unpickler::new(UnpicklerOptions::default());

        let comparison = get_test_object(major);
        let unpickled = unpickler.load(file).unwrap();
        assert_eq!(unpickled, comparison, "py {}, proto {}", major, proto);
    }
}

#[test]
fn recursive() {
    for proto in &[0, 1, 2, 3, 4, 5] {
        let filename = format!("tests/data/test_recursive_proto{}.pickle", proto);
        let file = File::open(filename).unwrap();
        let unpickler = Unpickler::new(UnpicklerOptions::default());

        match unpickler.load(file) {
            Err(Error::Syntax(ErrorCode::Recursive)) => {}
            _ => panic!("wrong/no error returned for recursive structure"),
        }
    }
}

#[test]
fn unresolvable_global() {
    let data = std::fs::read("tests/data/test_unresolvable_global.pickle").unwrap();
    let unpickler = Unpickler::new(UnpicklerOptions::default());

    assert!(unpickler.load_from_slice(&data).is_err());
}
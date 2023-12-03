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

use num_bigint::BigInt;
use std::collections::{HashMap, HashSet};
use std::{fs::File, io::BufReader};

use pickle_rs::{
    Error, ErrorCode, F64Wrapper, HashMapWrapper, HashSetWrapper, Unpickler, UnpicklerOptions,
    Value,
};

macro_rules! pyobj {
    (n=None)     => { Value::None };
    (b=True)     => { Value::Bool(true) };
    (b=False)    => { Value::Bool(false) };
    (i=$i:expr)  => { Value::I64($i) };
    (ii=$i:expr) => { Value::Int($i.clone()) };
    (f=$f:expr)  => { Value::F64(F64Wrapper($f)) };
    (bb=$b:expr) => { Value::Bytes($b.to_vec()) };
    (s=$s:expr)  => { Value::String($s.into()) };
    (t=($($m:ident=$v:tt),*))  => { Value::Tuple(vec![$(pyobj!($m=$v)),*]) };
    (l=[$($m:ident=$v:tt),*])  => { Value::List(vec![$(pyobj!($m=$v)),*]) };
    (ss=($($m:ident=$v:tt),*)) => { Value::Set(HashSetWrapper(HashSet::from_iter(vec![$(pyobj!($m=$v)),*]))) };
    (fs=($($m:ident=$v:tt),*)) => { Value::FrozenSet(HashSetWrapper(HashSet::from_iter(vec![$(pyobj!($m=$v)),*]))) };
    (d={$($km:ident=$kv:tt => $vm:ident=$vv:tt),*}) => {
        Value::Dict(HashMapWrapper(HashMap::from_iter(vec![$((pyobj!($km=$kv),
                                                pyobj!($vm=$vv))),*]))) };
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
    // Reproduces the test_object from test/data/generate.py.
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
                map.0.insert(pyobj!(i = 7), pyobj!(d={bb=b"attr" => i=5}));
            } else {
                map.0.insert(pyobj!(i = 7), pyobj!(d={s="attr" => i=5}));
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
        println!("Filename: {}", filename);

        let file = File::open(filename).unwrap();

        let comparison = get_test_object(major);
        let unpickled = Unpickler::value_from_reader(file, UnpicklerOptions::default()).unwrap();

        assert_eq!(unpickled, comparison, "py {}, proto {}", major, proto);
    }
}

#[test]
fn recursive() {
    for proto in &[0, 1, 2, 3, 4, 5] {
        let filename = format!("tests/data/test_recursive_proto{}.pickle", proto);
        let file = File::open(filename).unwrap();
        let unpickled = Unpickler::value_from_reader(file, UnpicklerOptions::default());

        match unpickled {
            Err(Error::Syntax(ErrorCode::Recursive)) => {}
            _ => panic!("wrong/no error returned for recursive structure"),
        }
    }
}

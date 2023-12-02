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

use std::{fs::File, io::BufReader};

use pickle_rs::{Error, ErrorCode, F64Wrapper, HashMapWrapper, Unpickler, UnpicklerOptions, Value};

// combinations of (python major, pickle proto) to test
const TEST_CASES: &[(u32, u32)] = &[
    // (2, 0),
    (2, 1),
    // (2, 2),
    // (3, 0),
    // (3, 1),
    // (3, 2),
    // (3, 3),
    // (3, 4),
    // (3, 5),
];

fn get_test_object(_pyver: u32) -> Value {
    Value::Dict(HashMapWrapper(
        vec![
            (
                Value::Bool(false),
                Value::Tuple(vec![Value::Bool(false), Value::Bool(true)]),
            ),
            (Value::F64(F64Wrapper(1.0)), Value::F64(F64Wrapper(1.0))),
            (
                Value::I128(100000000000000000000),
                Value::I128(100000000000000000000),
            ),
            (
                Value::Int(7),
                Value::Dict(HashMapWrapper(
                    vec![(Value::String("attr".to_string()), Value::Int(5))]
                        .into_iter()
                        .collect(),
                )),
            ),
            (Value::Int(10), Value::Int(100000)),
            (
                Value::Reduce(
                    Box::new(Value::Class(
                        "__builtin__".to_string(),
                        "frozenset".to_string(),
                    )),
                    Box::new(Value::Tuple(vec![Value::List(vec![
                        Value::Int(0),
                        Value::Int(42),
                    ])])),
                ),
                Value::Reduce(
                    Box::new(Value::Class(
                        "__builtin__".to_string(),
                        "frozenset".to_string(),
                    )),
                    Box::new(Value::Tuple(vec![Value::List(vec![
                        Value::Int(0),
                        Value::Int(42),
                    ])])),
                ),
            ),
            (
                Value::String("string".to_string()),
                Value::String("string".to_string()),
            ),
            (
                Value::Tuple(vec![Value::Int(1), Value::Int(2)]),
                Value::Tuple(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
            ),
            (Value::None, Value::None),
            (
                Value::String("bytes".to_string()),
                Value::String("bytes".to_string()),
            ),
            (
                Value::Tuple(vec![]),
                Value::List(vec![
                    Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
                    Value::Reduce(
                        Box::new(Value::Class("__builtin__".to_string(), "set".to_string())),
                        Box::new(Value::Tuple(vec![Value::List(vec![
                            Value::Int(0),
                            Value::Int(42),
                        ])])),
                    ),
                    Value::Dict(HashMapWrapper(vec![].into_iter().collect())),
                    Value::Reduce(
                        Box::new(Value::Class(
                            "__builtin__".to_string(),
                            "bytearray".to_string(),
                        )),
                        Box::new(Value::Tuple(vec![
                            Value::Bytes(vec![0, 85, 170, 255]),
                            Value::String("latin-1".to_string()),
                        ])),
                    ),
                ]),
            ),
        ]
        .into_iter()
        .collect(),
    ))
}

#[test]
fn unpickle_all() {
    for &(major, proto) in TEST_CASES {
        let filename = format!("tests/data/tests_py{}_proto{}.pickle", major, proto);
        println!("Filename: {}", filename);

        let file = BufReader::new(File::open(filename).unwrap());
        let mut unpickler = Unpickler::new(UnpicklerOptions::default());

        let comparison = get_test_object(major);
        let unpickled = unpickler.load(file).unwrap();
        assert_eq!(unpickled, comparison, "py {}, proto {}", major, proto);
    }
}

#[test]
fn recursive() {
    for proto in &[0, 1, 2, 3, 4, 5] {
        let filename = format!("tests/data/test_recursive_proto{}.pickle", proto);
        let file = BufReader::new(File::open(filename).unwrap());
        let mut unpickler = Unpickler::new(UnpicklerOptions::default());

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

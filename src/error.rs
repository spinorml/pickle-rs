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

use std::fmt;
use std::io;
use std::result;

#[derive(Clone, PartialEq, Debug)]
pub enum ErrorCode {
    /// Unsupported opcode
    Unsupported(char),
    /// EOF while parsing op argument
    EOFWhileParsing,
    /// Stack underflowed
    StackUnderflow,
    /// Length prefix found negative
    NegativeLength,
    /// String decoding as UTF-8 failed
    StringNotUTF8,
    /// Wrong stack top type for opcode
    InvalidStackTop(&'static str, String),
    /// Value not hashable, but used as dict key or set item
    ValueNotHashable,
    /// Recursive structure found, which we don't support
    Recursive,
    /// A "module global" reference wasn't resolved by REDUCE
    UnresolvedGlobal,
    /// A "module global" isn't supported
    UnsupportedGlobal(Vec<u8>, Vec<u8>),
    /// A value was missing from the memo
    MissingMemo(u32),
    /// Invalid literal found
    InvalidLiteral(Vec<u8>),
    /// Found trailing bytes after STOP opcode
    TrailingBytes,
    /// Invalid value in pickle stream
    InvalidValue(String),
    /// Structure deserialization error (e.g., unknown variant)
    Structure(String),
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ErrorCode::Unsupported(ch) => write!(fmt, "unsupported opcode {:?}", ch),
            ErrorCode::EOFWhileParsing => write!(fmt, "EOF while parsing"),
            ErrorCode::StackUnderflow => write!(fmt, "pickle stack underflow"),
            ErrorCode::NegativeLength => write!(fmt, "negative length prefix"),
            ErrorCode::StringNotUTF8 => write!(fmt, "string is not UTF-8 encoded"),
            ErrorCode::InvalidStackTop(what, ref it) => {
                write!(fmt, "invalid stack top, expected {}, got {}", what, it)
            }
            ErrorCode::ValueNotHashable => write!(fmt, "dict key or set item not hashable"),
            ErrorCode::Recursive => write!(fmt, "recursive structure found"),
            ErrorCode::UnresolvedGlobal => write!(fmt, "unresolved global reference"),
            ErrorCode::UnsupportedGlobal(ref m, ref g) => write!(
                fmt,
                "unsupported global: {}.{}",
                String::from_utf8_lossy(m),
                String::from_utf8_lossy(g)
            ),
            ErrorCode::MissingMemo(n) => write!(fmt, "missing memo with id {}", n),
            ErrorCode::InvalidLiteral(ref l) => {
                write!(fmt, "literal is invalid: {}", String::from_utf8_lossy(l))
            }
            ErrorCode::TrailingBytes => write!(fmt, "trailing bytes found"),
            ErrorCode::InvalidValue(ref s) => write!(fmt, "invalid value: {}", s),
            ErrorCode::Structure(ref s) => fmt.write_str(s),
        }
    }
}

/// This type represents all possible errors that can occur when serializing or
/// deserializing a value.
#[derive(Debug)]
pub enum Error {
    /// Some IO error occurred when serializing or deserializing a value.
    Io(io::Error),
    /// The pickle had some error while interpreting.
    Eval(ErrorCode, usize),
    /// Syntax error while transforming into Rust values.
    Syntax(ErrorCode),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Error {
        Error::Io(error)
    }
}

pub type Result<T> = result::Result<T, Error>;

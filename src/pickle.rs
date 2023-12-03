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

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::str::{self, FromStr};

use byteorder::{BigEndian, ByteOrder, LittleEndian};
use num_bigint::{BigInt, Sign};
use num_traits::ToPrimitive;

use crate::error::Result;
use crate::value::{Global, Value};
use crate::{Error, ErrorCode, F64Wrapper, HashMapWrapper, HashSetWrapper, MemoId};

const MARK: u8 = b'('; // push special markobject on stack
const STOP: u8 = b'.'; // every pickle ends with STOP
const POP: u8 = b'0'; // discard topmost stack item
const POP_MARK: u8 = b'1'; // discard stack top through topmost markobject
const DUP: u8 = b'2'; // duplicate top stack item
const FLOAT: u8 = b'F'; // push float object; decimal string argument
const INT: u8 = b'I'; // push integer or bool; decimal string argument
const BININT: u8 = b'J'; // push four-byte signed int
const BININT1: u8 = b'K'; // push 1-byte unsigned int
const LONG: u8 = b'L'; // push long; decimal string argument
const BININT2: u8 = b'M'; // push 2-byte unsigned int
const NONE: u8 = b'N'; // push None
const PERSID: u8 = b'P'; // push persistent object; id is taken from string arg
const BINPERSID: u8 = b'Q'; // " " " ;  "  "   "    "  stack
const REDUCE: u8 = b'R'; // apply callable to argtuple, both on stack
const STRING: u8 = b'S'; // push string; NL-terminated string argument
const BINSTRING: u8 = b'T'; // push string; counted binary string argument
const SHORT_BINSTRING: u8 = b'U'; // " " " ;    "      "       "      " < 256 bytes
const UNICODE: u8 = b'V'; // push Unicode string; raw-unicode-escaped'd argument
const BINUNICODE: u8 = b'X'; // " " " ; counted UTF-8 string argument
const APPEND: u8 = b'a'; // append stack top to list below it
const BUILD: u8 = b'b'; // call __setstate__ or __dict__.update()
const GLOBAL: u8 = b'c'; // push self.find_class(modname, name); 2 string args
const DICT: u8 = b'd'; // build a dict from stack items
const EMPTY_DICT: u8 = b'}'; // push empty dict
const APPENDS: u8 = b'e'; // extend list on stack by topmost stack slice
const GET: u8 = b'g'; // push item from memo on stack; index is string arg
const BINGET: u8 = b'h'; // " " " " " ;   "    " 1-byte arg
const INST: u8 = b'i'; // build & push class instance
const LONG_BINGET: u8 = b'j'; // push item from memo on stack; index is 4-byte arg
const LIST: u8 = b'l'; // build list from topmost stack items
const EMPTY_LIST: u8 = b']'; // push empty list
const OBJ: u8 = b'o'; // build & push class instance
const PUT: u8 = b'p'; // store stack top in memo; index is string arg
const BINPUT: u8 = b'q'; // " " " " " " ;   "    " 1-byte arg
const LONG_BINPUT: u8 = b'r'; // " " " " " " ;   "    " 4-byte arg
const SETITEM: u8 = b's'; // add key+value pair to dict
const TUPLE: u8 = b't'; // build tuple from topmost stack items
const EMPTY_TUPLE: u8 = b')'; // push empty tuple
const SETITEMS: u8 = b'u'; // modify dict by adding topmost key+value pairs
const BINFLOAT: u8 = b'G'; // push float; arg is 8-byte float encoding

// # Protocol 2
const PROTO: u8 = b'\x80'; // identify pickle protocol
const NEWOBJ: u8 = b'\x81'; // build object by applying cls.__new__ to argtuple
const EXT1: u8 = b'\x82'; // push object from extension registry; 1-byte index
const EXT2: u8 = b'\x83'; // ditto, but 2-byte index
const EXT4: u8 = b'\x84'; // ditto, but 4-byte index
const TUPLE1: u8 = b'\x85'; // build 1-tuple from stack top
const TUPLE2: u8 = b'\x86'; // build 2-tuple from two topmost stack items
const TUPLE3: u8 = b'\x87'; // build 3-tuple from three topmost stack items
const NEWTRUE: u8 = b'\x88'; // push True
const NEWFALSE: u8 = b'\x89'; // push False
const LONG1: u8 = b'\x8a'; // push long from < 256 bytes
const LONG4: u8 = b'\x8b'; // push really big long

// # Protocol 3 (Python 3.x)

const BINBYTES: u8 = b'B'; // push bytes; counted binary string argument
const SHORT_BINBYTES: u8 = b'C'; // < 256 bytes

// # Protocol 4

const SHORT_BINUNICODE: u8 = b'\x8c'; // push short string; UTF-8 length < 256 bytes
const BINUNICODE8: u8 = b'\x8d'; // push very long string
const BINBYTES8: u8 = b'\x8e'; // push very long bytes string
const EMPTY_SET: u8 = b'\x8f'; // push empty set on the stack
const ADDITEMS: u8 = b'\x90'; // modify set by adding topmost stack items
const FROZENSET: u8 = b'\x91'; // build frozenset from topmost stack items
const NEWOBJ_EX: u8 = b'\x92'; // like NEWOBJ but work with keyword only arguments
const STACK_GLOBAL: u8 = b'\x93'; // same as GLOBAL but using names on the stacks
const MEMOIZE: u8 = b'\x94'; // store top of the stack in memo
const FRAME: u8 = b'\x95'; // indicate the beginning of a new frame

// # Protocol 5

const BYTEARRAY8: u8 = b'\x96'; // push bytearray
const NEXT_BUFFER: u8 = b'\x97'; // push next out-of-band buffer
const READONLY_BUFFER: u8 = b'\x98'; // make top of stack readonly

const TRUE: &str = "01"; // not an opcode; see INT docs in pickletools.py
const FALSE: &str = "00"; // not an opcode; see INT docs in pickletools.py

pub struct UnpicklerOptions {
    fix_imports: bool,
    encoding: String,
    strict: bool,
    decode_strings: bool,
}

impl Default for UnpicklerOptions {
    fn default() -> Self {
        Self {
            fix_imports: true,
            encoding: "ASCII".to_string(),
            strict: true,
            decode_strings: true,
        }
    }
}

pub struct Unpickler<R: Read> {
    options: UnpicklerOptions,
    reader: BufReader<R>,
    metastack: Vec<Vec<Value>>,
    stack: Vec<Value>,
    memo: HashMap<MemoId, (Value, i32)>,
    pos: usize,
}

impl<R: Read> Unpickler<R> {
    pub fn new(reader: R, options: UnpicklerOptions) -> Self {
        Self {
            options,
            reader: BufReader::new(reader),
            metastack: Vec::new(),
            stack: Vec::new(),
            memo: HashMap::new(),
            pos: 0,
        }
    }

    /// Decodes a value from a `std::io::Read`.
    pub fn value_from_reader(rdr: R, options: UnpicklerOptions) -> Result<Value> {
        let mut unpickler = Unpickler::new(rdr, options);
        let value = unpickler.deserialize_value()?;
        unpickler.end()?;
        Ok(value)
    }

    fn deserialize_value(&mut self) -> Result<Value> {
        let internal_value = self.parse_value()?;
        self.convert_value(internal_value)
    }

    fn parse_value(&mut self) -> Result<Value> {
        loop {
            let byte = self.read_byte()?;
            match byte {
                // Specials
                PROTO => {
                    // Ignore this, as it is only important for instances (read
                    // the version byte).
                    self.read_byte()?;
                }
                FRAME => {
                    // We'll ignore framing. But we still have to gobble up the length.
                    self.read_fixed_8_bytes()?;
                }
                STOP => return self.pop(),
                MARK => {
                    let stack = std::mem::replace(&mut self.stack, Vec::with_capacity(128));
                    self.metastack.push(stack);
                }
                POP => {
                    if self.stack.is_empty() {
                        self.pop_mark()?;
                    } else {
                        self.pop()?;
                    }
                }
                POP_MARK => {
                    self.pop_mark()?;
                }
                DUP => {
                    let top = self.top()?.clone();
                    self.stack.push(top);
                }

                // Memo saving ops
                PUT => {
                    let bytes = self.read_line()?;
                    let memo_id = self.parse_ascii(bytes)?;
                    self.memoize(memo_id)?;
                }
                BINPUT => {
                    let memo_id = self.read_byte()?;
                    self.memoize(memo_id.into())?;
                }
                LONG_BINPUT => {
                    let bytes = self.read_fixed_4_bytes()?;
                    let memo_id = LittleEndian::read_u32(&bytes);
                    self.memoize(memo_id)?;
                }
                MEMOIZE => {
                    let memo_id = self.memo.len();
                    self.memoize(memo_id as MemoId)?;
                }

                // Memo getting ops
                GET => {
                    let bytes = self.read_line()?;
                    let memo_id = self.parse_ascii(bytes)?;
                    self.push_memo_ref(memo_id)?;
                }
                BINGET => {
                    let memo_id = self.read_byte()?;
                    self.push_memo_ref(memo_id.into())?;
                }
                LONG_BINGET => {
                    let bytes = self.read_fixed_4_bytes()?;
                    let memo_id = LittleEndian::read_u32(&bytes);
                    self.push_memo_ref(memo_id)?;
                }

                // Singletons
                NONE => self.stack.push(Value::None),
                NEWFALSE => self.stack.push(Value::Bool(false)),
                NEWTRUE => self.stack.push(Value::Bool(true)),

                // ASCII-formatted numbers
                INT => {
                    let line = self.read_line()?;
                    let val = self.decode_text_int(line)?;
                    self.stack.push(val);
                }
                LONG => {
                    let line = self.read_line()?;
                    let long = self.decode_text_long(line)?;
                    self.stack.push(long);
                }
                FLOAT => {
                    let line = self.read_line()?;
                    let f = F64Wrapper(self.parse_ascii(line)?);
                    self.stack.push(Value::F64(f));
                }

                // ASCII-formatted strings
                STRING => {
                    let line = self.read_line()?;
                    let string = self.decode_escaped_string(&line)?;
                    self.stack.push(string);
                }
                UNICODE => {
                    let line = self.read_line()?;
                    let string = self.decode_escaped_unicode(&line)?;
                    self.stack.push(string);
                }

                // Binary-coded numbers
                BINFLOAT => {
                    let bytes = self.read_fixed_8_bytes()?;
                    self.stack
                        .push(Value::F64(F64Wrapper(BigEndian::read_f64(&bytes))));
                }
                BININT => {
                    let bytes = self.read_fixed_4_bytes()?;
                    self.stack
                        .push(Value::I64(LittleEndian::read_i32(&bytes).into()));
                }
                BININT1 => {
                    let byte = self.read_byte()?;
                    self.stack.push(Value::I64(byte.into()));
                }
                BININT2 => {
                    let bytes = self.read_fixed_2_bytes()?;
                    self.stack
                        .push(Value::I64(LittleEndian::read_u16(&bytes).into()));
                }
                LONG1 => {
                    let bytes = self.read_u8_prefixed_bytes()?;
                    let long = self.decode_binary_long(bytes);
                    self.stack.push(long);
                }
                LONG4 => {
                    let bytes = self.read_i32_prefixed_bytes()?;
                    let long = self.decode_binary_long(bytes);
                    self.stack.push(long);
                }

                // Length-prefixed (byte)strings
                SHORT_BINBYTES => {
                    let string = self.read_u8_prefixed_bytes()?;
                    self.stack.push(Value::Bytes(string));
                }
                BINBYTES => {
                    let string = self.read_u32_prefixed_bytes()?;
                    self.stack.push(Value::Bytes(string));
                }
                BINBYTES8 => {
                    let string = self.read_u64_prefixed_bytes()?;
                    self.stack.push(Value::Bytes(string));
                }
                SHORT_BINSTRING => {
                    let string = self.read_u8_prefixed_bytes()?;
                    let decoded = self.decode_string(string)?;
                    self.stack.push(decoded);
                }
                BINSTRING => {
                    let string = self.read_i32_prefixed_bytes()?;
                    let decoded = self.decode_string(string)?;
                    self.stack.push(decoded);
                }
                SHORT_BINUNICODE => {
                    let string = self.read_u8_prefixed_bytes()?;
                    let decoded = self.decode_unicode(string)?;
                    self.stack.push(decoded);
                }
                BINUNICODE => {
                    println!("BINUNICODE");
                    let string = self.read_u32_prefixed_bytes()?;
                    let decoded = self.decode_unicode(string)?;
                    println!("BINUNICODE - decoded: {:?}", decoded);
                    self.stack.push(decoded);
                }
                BINUNICODE8 => {
                    let string = self.read_u64_prefixed_bytes()?;
                    let decoded = self.decode_unicode(string)?;
                    self.stack.push(decoded);
                }
                BYTEARRAY8 => {
                    let string = self.read_u64_prefixed_bytes()?;
                    self.stack.push(Value::Bytes(string));
                }

                // Tuples
                EMPTY_TUPLE => self.stack.push(Value::Tuple(Vec::new())),
                TUPLE1 => {
                    let item = self.pop()?;
                    self.stack.push(Value::Tuple(vec![item]));
                }
                TUPLE2 => {
                    let item2 = self.pop()?;
                    let item1 = self.pop()?;
                    self.stack.push(Value::Tuple(vec![item1, item2]));
                }
                TUPLE3 => {
                    let item3 = self.pop()?;
                    let item2 = self.pop()?;
                    let item1 = self.pop()?;
                    self.stack.push(Value::Tuple(vec![item1, item2, item3]));
                }
                TUPLE => {
                    let items = self.pop_mark()?;
                    self.stack.push(Value::Tuple(items));
                }

                // Lists
                EMPTY_LIST => self.stack.push(Value::List(Vec::new())),
                LIST => {
                    let items = self.pop_mark()?;
                    self.stack.push(Value::List(items));
                }
                APPEND => {
                    let value = self.pop()?;
                    self.modify_list(|list| list.push(value))?;
                }
                APPENDS => {
                    let items = self.pop_mark()?;
                    self.modify_list(|list| list.extend(items))?;
                }

                // Dicts
                EMPTY_DICT => self.stack.push(Value::Dict(HashMapWrapper(HashMap::new()))),
                DICT => {
                    let items = self.pop_mark()?;
                    let mut dict = HashMap::with_capacity(items.len() / 2);
                    for chunk in items.chunks_exact(2) {
                        dict.insert(chunk[0].clone(), chunk[1].clone());
                    }
                    self.stack.push(Value::Dict(HashMapWrapper(dict)));
                }
                SETITEM => {
                    let value = self.pop()?;
                    let key = self.pop()?;
                    self.modify_dict(|dict| {
                        dict.insert(key, value);
                    })?;
                }
                SETITEMS => {
                    let items = self.pop_mark()?;
                    self.modify_dict(|dict| {
                        for chunk in items.chunks_exact(2) {
                            dict.insert(chunk[0].clone(), chunk[1].clone());
                        }
                    })?;
                }

                // Sets and frozensets
                EMPTY_SET => self.stack.push(Value::Set(HashSetWrapper(HashSet::new()))),
                FROZENSET => {
                    let items = self.pop_mark()?;
                    self.stack.push(Value::FrozenSet(HashSetWrapper(
                        items.into_iter().collect(),
                    )));
                }
                ADDITEMS => {
                    let items = self.pop_mark()?;
                    self.modify_set(|set| set.extend(items))?;
                }

                // Arbitrary module globals, used here for unpickling set and frozenset
                // from protocols < 4
                GLOBAL => {
                    let modname = self.read_line()?;
                    let globname = self.read_line()?;
                    let value = self.decode_global(modname, globname)?;
                    self.stack.push(value);
                }
                STACK_GLOBAL => {
                    let globname = match self.pop_resolve()? {
                        Value::String(string) => string.into_bytes(),
                        other => return Self::stack_error("string", &other, self.pos),
                    };
                    let modname = match self.pop_resolve()? {
                        Value::String(string) => string.into_bytes(),
                        other => return Self::stack_error("string", &other, self.pos),
                    };
                    let value = self.decode_global(modname, globname)?;
                    self.stack.push(value);
                }
                REDUCE => {
                    let argtuple = match self.pop_resolve()? {
                        Value::Tuple(args) => args,
                        other => return Self::stack_error("tuple", &other, self.pos),
                    };
                    let global = self.pop_resolve()?;
                    self.reduce_global(global, argtuple)?;
                }

                // Arbitrary classes - make a best effort attempt to recover some data
                INST => {
                    // pop module name and class name
                    for _ in 0..2 {
                        self.read_line()?;
                    }
                    // pop arguments to init
                    self.pop_mark()?;
                    // push empty dictionary instead of the class instance
                    self.stack.push(Value::Dict(HashMapWrapper(HashMap::new())));
                }
                OBJ => {
                    // pop arguments to init
                    self.pop_mark()?;
                    // pop class object
                    self.pop()?;
                    self.stack.push(Value::Dict(HashMapWrapper(HashMap::new())));
                }
                NEWOBJ => {
                    // pop arguments and class object
                    for _ in 0..2 {
                        self.pop()?;
                    }
                    self.stack.push(Value::Dict(HashMapWrapper(HashMap::new())));
                }
                NEWOBJ_EX => {
                    // pop keyword args, arguments and class object
                    for _ in 0..3 {
                        self.pop()?;
                    }
                    self.stack.push(Value::Dict(HashMapWrapper(HashMap::new())));
                }
                BUILD => {
                    // The top-of-stack for BUILD is used either as the instance __dict__,
                    // or an argument for __setstate__, in which case it can be *any* type
                    // of object.  In both cases, we just replace the standin.
                    let state = self.pop()?;
                    self.pop()?; // remove the object standin
                    self.stack.push(state);
                }

                PERSID => {
                    let line = self.read_line()?;
                    println!("PERSID: {:?}", line);
                    let bytes = Value::Bytes(line);
                    self.stack.push(Value::BinPersId(Box::new(bytes)));
                }

                BINPERSID => {
                    let binpers_id = self.pop()?;
                    self.stack.push(Value::BinPersId(Box::new(binpers_id)));
                }

                // Unsupported opcodes
                code => return self.error(ErrorCode::Unsupported(code as char)),
            }
        }
    }

    // Pop the stack top item.
    fn pop(&mut self) -> Result<Value> {
        match self.stack.pop() {
            Some(v) => Ok(v),
            None => self.error(ErrorCode::StackUnderflow),
        }
    }

    // Pop the stack top item, and resolve it if it is a memo reference.
    fn pop_resolve(&mut self) -> Result<Value> {
        let top = self.stack.pop();
        match self.resolve(top) {
            Some(v) => Ok(v),
            None => self.error(ErrorCode::StackUnderflow),
        }
    }

    // Pop all topmost stack items until the next MARK.
    fn pop_mark(&mut self) -> Result<Vec<Value>> {
        match self.metastack.pop() {
            Some(new) => Ok(std::mem::replace(&mut self.stack, new)),
            None => self.error(ErrorCode::StackUnderflow),
        }
    }

    // Mutably view the stack top item.
    fn top(&mut self) -> Result<&mut Value> {
        match self.stack.last_mut() {
            // Since some operations like APPEND do things to the stack top, we
            // need to provide the reference to the "real" object here, not the
            // MemoRef variant.
            Some(&mut Value::MemoRef(n)) => self
                .memo
                .get_mut(&n)
                .map(|&mut (ref mut v, _)| v)
                .ok_or_else(|| Error::Syntax(ErrorCode::MissingMemo(n))),
            Some(other_value) => Ok(other_value),
            None => Err(Error::Eval(ErrorCode::StackUnderflow, self.pos)),
        }
    }

    // Pushes a memo reference on the stack, and increases the usage counter.
    fn push_memo_ref(&mut self, memo_id: MemoId) -> Result<()> {
        self.stack.push(Value::MemoRef(memo_id));
        match self.memo.get_mut(&memo_id) {
            Some(&mut (_, ref mut count)) => {
                *count += 1;
                Ok(())
            }
            None => Err(Error::Eval(ErrorCode::MissingMemo(memo_id), self.pos)),
        }
    }

    // Memoize the current stack top with the given ID.  Moves the actual
    // object into the memo, and saves a reference on the stack instead.
    fn memoize(&mut self, memo_id: MemoId) -> Result<()> {
        let mut item = self.pop()?;
        if let Value::MemoRef(id) = item {
            // TODO: is this even possible?
            item = match self.memo.get(&id) {
                Some((v, _)) => v.clone(),
                None => return Err(Error::Eval(ErrorCode::MissingMemo(id), self.pos)),
            };
        }
        self.memo.insert(memo_id, (item, 1));
        self.stack.push(Value::MemoRef(memo_id));
        Ok(())
    }

    // Resolve memo reference during stream decoding.
    fn resolve(&mut self, maybe_memo: Option<Value>) -> Option<Value> {
        match maybe_memo {
            Some(Value::MemoRef(id)) => {
                self.memo.get_mut(&id).map(|&mut (ref val, ref mut count)| {
                    // We can't remove it from the memo here, since we haven't
                    // decoded the whole stream yet and there may be further
                    // references to the value.
                    *count -= 1;
                    val.clone()
                })
            }
            other => other,
        }
    }

    // Resolve memo reference during Value deserializing.
    fn resolve_recursive<T, U, F>(&mut self, id: MemoId, u: U, f: F) -> Result<T>
    where
        F: FnOnce(&mut Self, U, Value) -> Result<T>,
    {
        // Take the value from the memo while visiting it.  This prevents us
        // from trying to depickle recursive structures, which we can't do
        // because our Values aren't references.
        let (value, mut count) = match self.memo.remove(&id) {
            Some(entry) => entry,
            None => return Err(Error::Syntax(ErrorCode::Recursive)),
        };
        count -= 1;
        if count <= 0 {
            f(self, u, value)
            // No need to put it back.
        } else {
            let result = f(self, u, value.clone());
            self.memo.insert(id, (value, count));
            result
        }
    }

    /// Assert that we reached the end of the stream.
    fn end(&mut self) -> Result<()> {
        let mut buf = [0];
        match self.reader.read(&mut buf) {
            Err(err) => Err(Error::Io(err)),
            Ok(1) => self.error(ErrorCode::TrailingBytes),
            _ => Ok(()),
        }
    }

    fn read_line(&mut self) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(16);
        match self.reader.read_until(b'\n', &mut buf) {
            Ok(_) => {
                self.pos += buf.len();
                buf.pop(); // remove newline
                if buf.last() == Some(&b'\r') {
                    buf.pop();
                }
                Ok(buf)
            }
            Err(err) => Err(Error::Io(err)),
        }
    }

    #[inline]
    fn read_byte(&mut self) -> Result<u8> {
        let mut buf = [0];
        match self.reader.read(&mut buf) {
            Ok(1) => {
                self.pos += 1;
                Ok(buf[0])
            }
            Ok(_) => self.error(ErrorCode::EOFWhileParsing),
            Err(err) => Err(Error::Io(err)),
        }
    }

    #[inline]
    fn read_bytes(&mut self, n: usize) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        match self.reader.by_ref().take(n as u64).read_to_end(&mut buf) {
            Ok(m) if n == m => {
                self.pos += n;
                Ok(buf)
            }
            Ok(_) => self.error(ErrorCode::EOFWhileParsing),
            Err(err) => Err(Error::Io(err)),
        }
    }

    #[inline]
    fn read_fixed_2_bytes(&mut self) -> Result<[u8; 2]> {
        let mut buf = [0; 2];
        match self.reader.by_ref().take(2).read_exact(&mut buf) {
            Ok(()) => {
                self.pos += 2;
                Ok(buf)
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::UnexpectedEof {
                    self.error(ErrorCode::EOFWhileParsing)
                } else {
                    Err(Error::Io(err))
                }
            }
        }
    }

    #[inline]
    fn read_fixed_4_bytes(&mut self) -> Result<[u8; 4]> {
        let mut buf = [0; 4];
        match self.reader.by_ref().take(4).read_exact(&mut buf) {
            Ok(()) => {
                self.pos += 4;
                Ok(buf)
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::UnexpectedEof {
                    self.error(ErrorCode::EOFWhileParsing)
                } else {
                    Err(Error::Io(err))
                }
            }
        }
    }

    #[inline]
    fn read_fixed_8_bytes(&mut self) -> Result<[u8; 8]> {
        let mut buf = [0; 8];
        match self.reader.by_ref().take(8).read_exact(&mut buf) {
            Ok(()) => {
                self.pos += 8;
                Ok(buf)
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::UnexpectedEof {
                    self.error(ErrorCode::EOFWhileParsing)
                } else {
                    Err(Error::Io(err))
                }
            }
        }
    }

    fn read_i32_prefixed_bytes(&mut self) -> Result<Vec<u8>> {
        let lenbytes = self.read_fixed_4_bytes()?;
        match LittleEndian::read_i32(&lenbytes) {
            0 => Ok(vec![]),
            l if l < 0 => self.error(ErrorCode::NegativeLength),
            l => self.read_bytes(l as usize),
        }
    }

    fn read_u64_prefixed_bytes(&mut self) -> Result<Vec<u8>> {
        let lenbytes = self.read_fixed_8_bytes()?;
        self.read_bytes(LittleEndian::read_u64(&lenbytes) as usize)
    }

    fn read_u32_prefixed_bytes(&mut self) -> Result<Vec<u8>> {
        let lenbytes = self.read_fixed_4_bytes()?;
        println!("read_u32_prefixed_bytes - lenbytes: {:?}", lenbytes);
        self.read_bytes(LittleEndian::read_u32(&lenbytes) as usize)
    }

    fn read_u8_prefixed_bytes(&mut self) -> Result<Vec<u8>> {
        let lenbyte = self.read_byte()?;
        println!("read_u8_prefixed_bytes - lenbyte: {}", lenbyte);
        self.read_bytes(lenbyte as usize)
    }

    // Parse an expected ASCII literal from the stream or raise an error.
    fn parse_ascii<T: FromStr>(&self, bytes: Vec<u8>) -> Result<T> {
        match str::from_utf8(&bytes).unwrap_or("").parse() {
            Ok(v) => Ok(v),
            Err(_) => self.error(ErrorCode::InvalidLiteral(bytes)),
        }
    }

    // Decode a text-encoded integer.
    fn decode_text_int(&self, line: Vec<u8>) -> Result<Value> {
        // Handle protocol 1 way of spelling true/false
        Ok(if line == b"00" {
            Value::Bool(false)
        } else if line == b"01" {
            Value::Bool(true)
        } else {
            let i = self.parse_ascii(line)?;
            Value::I64(i)
        })
    }

    // Decode a text-encoded long integer.
    fn decode_text_long(&self, mut line: Vec<u8>) -> Result<Value> {
        // Remove "L" suffix.
        if line.last() == Some(&b'L') {
            line.pop();
        }
        match BigInt::parse_bytes(&line, 10) {
            Some(i) => Ok(Value::Int(i)),
            None => self.error(ErrorCode::InvalidLiteral(line)),
        }
    }

    // Decode an escaped string.  These are encoded with "normal" Python string
    // escape rules.
    fn decode_escaped_string(&self, slice: &[u8]) -> Result<Value> {
        // Remove quotes if they appear.
        let slice = if (slice.len() >= 2)
            && (slice[0] == slice[slice.len() - 1])
            && (slice[0] == b'"' || slice[0] == b'\'')
        {
            &slice[1..slice.len() - 1]
        } else {
            slice
        };
        let mut result = Vec::with_capacity(slice.len());
        let mut iter = slice.iter();
        while let Some(&b) = iter.next() {
            match b {
                b'\\' => match iter.next() {
                    Some(&b'\\') => result.push(b'\\'),
                    Some(&b'a') => result.push(b'\x07'),
                    Some(&b'b') => result.push(b'\x08'),
                    Some(&b't') => result.push(b'\x09'),
                    Some(&b'n') => result.push(b'\x0a'),
                    Some(&b'v') => result.push(b'\x0b'),
                    Some(&b'f') => result.push(b'\x0c'),
                    Some(&b'r') => result.push(b'\x0d'),
                    Some(&b'x') => {
                        match iter
                            .next()
                            .and_then(|&ch1| (ch1 as char).to_digit(16))
                            .and_then(|v1| {
                                iter.next()
                                    .and_then(|&ch2| (ch2 as char).to_digit(16))
                                    .map(|v2| 16 * (v1 as u8) + (v2 as u8))
                            }) {
                            Some(v) => result.push(v),
                            None => return self.error(ErrorCode::InvalidLiteral(slice.into())),
                        }
                    }
                    _ => return self.error(ErrorCode::InvalidLiteral(slice.into())),
                },
                _ => result.push(b),
            }
        }
        self.decode_string(result)
    }

    // Decode escaped Unicode strings. These are encoded with "raw-unicode-escape",
    // which only knows the \uXXXX and \UYYYYYYYY escapes. The backslash is escaped
    // in this way, too.
    fn decode_escaped_unicode(&self, s: &[u8]) -> Result<Value> {
        let mut result = String::with_capacity(s.len());
        let mut iter = s.iter();
        while let Some(&b) = iter.next() {
            match b {
                b'\\' => {
                    let nescape = match iter.next() {
                        Some(&b'u') => 4,
                        Some(&b'U') => 8,
                        _ => return self.error(ErrorCode::InvalidLiteral(s.into())),
                    };
                    let mut accum = 0;
                    for _i in 0..nescape {
                        accum *= 16;
                        match iter.next().and_then(|&ch| (ch as char).to_digit(16)) {
                            Some(v) => accum += v,
                            None => return self.error(ErrorCode::InvalidLiteral(s.into())),
                        }
                    }
                    match char::from_u32(accum) {
                        Some(v) => result.push(v),
                        None => return self.error(ErrorCode::InvalidLiteral(s.into())),
                    }
                }
                _ => result.push(b as char),
            }
        }
        Ok(Value::String(result))
    }

    // Decode a string - either as Unicode or as bytes.
    fn decode_string(&self, string: Vec<u8>) -> Result<Value> {
        if self.options.decode_strings {
            self.decode_unicode(string)
        } else {
            Ok(Value::Bytes(string))
        }
    }

    // Decode a Unicode string from UTF-8.
    fn decode_unicode(&self, string: Vec<u8>) -> Result<Value> {
        match String::from_utf8(string) {
            Ok(v) => Ok(Value::String(v)),
            Err(_) => self.error(ErrorCode::StringNotUTF8),
        }
    }

    // Decode a binary-encoded long integer.
    fn decode_binary_long(&self, bytes: Vec<u8>) -> Value {
        // BigInt::from_bytes_le doesn't like a sign bit in the bytes, therefore
        // we have to extract that ourselves and do the two-s complement.
        let negative = !bytes.is_empty() && (bytes[bytes.len() - 1] & 0x80 != 0);
        let mut val = BigInt::from_bytes_le(Sign::Plus, &bytes);
        if negative {
            val -= BigInt::from(1) << (bytes.len() * 8);
        }
        Value::Int(val)
    }

    // Modify the stack-top list.
    fn modify_list<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Vec<Value>),
    {
        let pos = self.pos;
        let top = self.top()?;
        if let Value::List(ref mut list) = *top {
            f(list);
            Ok(())
        } else {
            Self::stack_error("list", top, pos)
        }
    }

    // Push items from a (key, value, key, value) flattened list onto a (key, value) vec.
    fn extend_dict(dict: &mut Vec<(Value, Value)>, items: Vec<Value>) {
        let mut key = None;
        for value in items {
            match key.take() {
                None => key = Some(value),
                Some(key) => dict.push((key, value)),
            }
        }
    }

    // Modify the stack-top dict.
    fn modify_dict<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut HashMap<Value, Value>),
    {
        let pos = self.pos;
        let top = self.top()?;
        if let Value::Dict(ref mut dict) = *top {
            f(&mut dict.0);
            Ok(())
        } else {
            Self::stack_error("dict", top, pos)
        }
    }

    // Modify the stack-top set.
    fn modify_set<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut HashSet<Value>),
    {
        let pos = self.pos;
        let top = self.top()?;
        if let Value::Set(ref mut set) = *top {
            f(&mut set.0);
            Ok(())
        } else {
            Self::stack_error("set", top, pos)
        }
    }

    // Push the Value::Global referenced by modname and globname.
    fn decode_global(&mut self, modname: Vec<u8>, globname: Vec<u8>) -> Result<Value> {
        let value = match (&*modname, &*globname) {
            (b"_codecs", b"encode") => Value::Global(Global::Encode),
            (b"__builtin__", b"set") | (b"builtins", b"set") => Value::Global(Global::Set),
            (b"__builtin__", b"frozenset") | (b"builtins", b"frozenset") => {
                Value::Global(Global::Frozenset)
            }
            (b"__builtin__", b"list") | (b"builtins", b"list") => Value::Global(Global::List),
            (b"__builtin__", b"bytearray") | (b"builtins", b"bytearray") => {
                Value::Global(Global::Bytearray)
            }
            (b"__builtin__", b"int") | (b"builtins", b"int") => Value::Global(Global::Int),
            _ => Value::Global(Global::Other),
        };
        Ok(value)
    }

    // Handle the REDUCE opcode for the few Global objects we support.
    fn reduce_global(&mut self, global: Value, mut argtuple: Vec<Value>) -> Result<()> {
        match global {
            Value::Global(Global::Set) => match self.resolve(argtuple.pop()) {
                Some(Value::List(items)) => {
                    self.stack
                        .push(Value::Set(HashSetWrapper(items.into_iter().collect())));
                    Ok(())
                }
                _ => self.error(ErrorCode::InvalidValue("set() arg".into())),
            },
            Value::Global(Global::Frozenset) => match self.resolve(argtuple.pop()) {
                Some(Value::List(items)) => {
                    self.stack.push(Value::FrozenSet(HashSetWrapper(
                        items.into_iter().collect(),
                    )));
                    Ok(())
                }
                _ => self.error(ErrorCode::InvalidValue("frozenset() arg".into())),
            },
            Value::Global(Global::Bytearray) => {
                // On Py2, the call is encoded as bytearray(u"foo", "latin-1").
                argtuple.truncate(1);
                match self.resolve(argtuple.pop()) {
                    Some(Value::Bytes(bytes)) => {
                        self.stack.push(Value::Bytes(bytes));
                        Ok(())
                    }
                    Some(Value::String(string)) => {
                        // The code points in the string are actually bytes values.
                        // So we need to collect them individually.
                        self.stack.push(Value::Bytes(
                            string.chars().map(|ch| ch as u32 as u8).collect(),
                        ));
                        Ok(())
                    }
                    _ => self.error(ErrorCode::InvalidValue("bytearray() arg".into())),
                }
            }
            Value::Global(Global::List) => match self.resolve(argtuple.pop()) {
                Some(Value::List(items)) => {
                    self.stack.push(Value::List(items));
                    Ok(())
                }
                _ => self.error(ErrorCode::InvalidValue("list() arg".into())),
            },
            Value::Global(Global::Int) => match self.resolve(argtuple.pop()) {
                Some(Value::Int(integer)) => {
                    self.stack.push(Value::Int(integer));
                    Ok(())
                }
                _ => self.error(ErrorCode::InvalidValue("int() arg".into())),
            },
            Value::Global(Global::Encode) => {
                // Byte object encoded as _codecs.encode(x, 'latin1')
                match self.resolve(argtuple.pop()) {
                    // Encoding, always latin1
                    Some(Value::String(_)) => {}
                    _ => return self.error(ErrorCode::InvalidValue("encode() arg".into())),
                }
                match self.resolve(argtuple.pop()) {
                    Some(Value::String(s)) => {
                        // Now we have to convert the string to latin-1
                        // encoded bytes.  It never contains codepoints
                        // above 0xff.
                        let bytes = s.chars().map(|ch| ch as u8).collect();
                        self.stack.push(Value::Bytes(bytes));
                        Ok(())
                    }
                    _ => self.error(ErrorCode::InvalidValue("encode() arg".into())),
                }
            }
            Value::Global(Global::Other) => {
                // Anything else; just keep it on the stack as an opaque object.
                // If it is a class object, it will get replaced later when the
                // class is instantiated.
                self.stack.push(Value::Global(Global::Other));
                Ok(())
            }
            other => Self::stack_error("global reference", &other, self.pos),
        }
    }

    fn convert_value(&mut self, value: Value) -> Result<Value> {
        match value {
            Value::Int(v) => {
                if let Some(i) = v.to_i64() {
                    Ok(Value::I64(i))
                } else {
                    Ok(Value::Int(v))
                }
            }
            Value::List(v) => {
                let new = v
                    .into_iter()
                    .map(|v| self.convert_value(v))
                    .collect::<Result<_>>();
                Ok(Value::List(new?))
            }
            Value::Tuple(v) => {
                let new = v
                    .into_iter()
                    .map(|v| self.convert_value(v))
                    .collect::<Result<_>>();
                Ok(Value::Tuple(new?))
            }
            Value::Set(v) => {
                let new =
                    v.0.into_iter()
                        .map(|v| self.convert_value(v))
                        .collect::<Result<_>>();
                Ok(Value::Set(HashSetWrapper(new?)))
            }
            Value::FrozenSet(v) => {
                let new =
                    v.0.into_iter()
                        .map(|v| self.convert_value(v))
                        .collect::<Result<_>>();
                Ok(Value::FrozenSet(HashSetWrapper(new?)))
            }
            Value::Dict(v) => {
                let mut map = HashMap::new();
                for (key, value) in v.0 {
                    let real_key = self.convert_value(key)?;
                    let real_value = self.convert_value(value)?;
                    map.insert(real_key, real_value);
                }
                Ok(Value::Dict(HashMapWrapper(map)))
            }
            Value::MemoRef(memo_id) => {
                self.resolve_recursive(memo_id, (), |slf, (), value| slf.convert_value(value))
            }
            _ => Ok(value),
        }
    }

    fn stack_error<T>(what: &'static str, value: &Value, pos: usize) -> Result<T> {
        let it = format!("{:?}", value);
        Err(Error::Eval(ErrorCode::InvalidStackTop(what, it), pos))
    }

    fn error<T>(&self, reason: ErrorCode) -> Result<T> {
        Err(Error::Eval(reason, self.pos))
    }
}

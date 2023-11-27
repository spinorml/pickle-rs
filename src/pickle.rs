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

use std::collections::HashMap;
use std::io::BufRead;

use byteorder::{BigEndian, ByteOrder, LittleEndian};

use crate::error::Result;
use crate::value::Value;
use crate::{Error, ErrorCode};

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

const TRUE: &[u8; 4] = b"I01\n"; // not an opcode; see INT docs in pickletools.py
const FALSE: &[u8; 4] = b"I00\n"; // not an opcode; see INT docs in pickletools.py

pub struct UnpicklerOptions {
    fix_imports: bool,
    encoding: String,
    strict: bool,
}

impl Default for UnpicklerOptions {
    fn default() -> Self {
        Self {
            fix_imports: true,
            encoding: "ASCII".to_string(),
            strict: true,
        }
    }
}

pub struct Unpickler {
    options: UnpicklerOptions,
    metastack: Vec<Vec<Value>>,
    stack: Vec<Value>,
    memo: HashMap<u32, Value>,
    proto: u8,
}

impl Unpickler {
    pub fn new(options: UnpicklerOptions) -> Self {
        Self {
            options,
            metastack: Vec::new(),
            stack: Vec::new(),
            memo: HashMap::new(),
            proto: 0,
        }
    }

    pub fn load(&mut self, mut file: impl BufRead) -> Result<Value> {
        let mut buf = [0; 1];
        let result;

        loop {
            file.read_exact(&mut buf)?;

            let op = buf[0];
            match op {
                STOP => {
                    result = self.load_stop();
                    break;
                }
                _ => {
                    self.load_op(op, &mut file)?;
                }
            }
        }

        match result {
            Some(result) => Ok(result),
            None => Err(Error::Syntax(ErrorCode::EOFWhileParsing)),
        }
    }

    pub fn load_from_slice(&self, _file: &[u8]) -> Result<Value> {
        todo!("Unpickler::load_from_slice")
    }

    fn load_op(&mut self, op: u8, mut file: impl BufRead) -> Result<()> {
        match op {
            MARK => self.load_mark(),
            POP => self.load_pop(),
            POP_MARK => self.load_pop_mark(),
            DUP => self.load_dup(),
            FLOAT => self.load_float(&mut file),
            INT => self.load_int(&mut file),
            BININT => self.load_binint(&mut file),
            BININT1 => self.load_binint1(&mut file),
            LONG => self.load_long(&mut file),
            BININT2 => self.load_binint2(&mut file),
            NONE => self.load_none(),
            PERSID => self.load_persid(&mut file),
            BINPERSID => self.load_binpersid(),
            REDUCE => self.load_reduce(),
            STRING => self.load_string(&mut file),
            BINSTRING => self.load_binstring(&mut file),
            SHORT_BINSTRING => self.load_short_binstring(&mut file),
            UNICODE => self.load_unicode(&mut file),
            BINUNICODE => self.load_binunicode(&mut file),
            APPEND => self.load_append(),
            BUILD => self.load_build(),
            GLOBAL => self.load_global(),
            DICT => self.load_dict(),
            EMPTY_DICT => self.load_empty_dict(),
            APPENDS => self.load_appends(),
            GET => self.load_get(),
            BINGET => self.load_binget(),
            INST => self.load_inst(),
            LONG_BINGET => self.load_long_binget(),
            LIST => self.load_list(),
            EMPTY_LIST => self.load_empty_list(),
            OBJ => self.load_obj(),
            PUT => self.load_put(&mut file),
            BINPUT => self.load_binput(),
            LONG_BINPUT => self.load_long_binput(),
            SETITEM => self.load_setitem(),
            TUPLE => self.load_tuple(),
            EMPTY_TUPLE => self.load_empty_tuple(),
            SETITEMS => self.load_setitems(),
            BINFLOAT => self.load_binfloat(),
            PROTO => self.load_proto(&mut file),
            NEWOBJ => self.load_newobj(),
            EXT1 => self.load_ext1(),
            EXT2 => self.load_ext2(),
            EXT4 => self.load_ext4(),
            TUPLE1 => self.load_tuple1(),
            TUPLE2 => self.load_tuple2(),
            TUPLE3 => self.load_tuple3(),
            NEWTRUE => self.load_newtrue(),
            NEWFALSE => self.load_newfalse(),
            LONG1 => self.load_long1(),
            LONG4 => self.load_long4(),
            BINBYTES => self.load_binbytes(),
            SHORT_BINBYTES => self.load_short_binbytes(),
            SHORT_BINUNICODE => self.load_short_binunicode(),
            BINUNICODE8 => self.load_binunicode8(),
            BINBYTES8 => self.load_binbytes8(),
            EMPTY_SET => self.load_empty_set(),
            ADDITEMS => self.load_additems(),
            FROZENSET => self.load_frozenset(),
            NEWOBJ_EX => self.load_newobj_ex(),
            STACK_GLOBAL => self.load_stack_global(),
            MEMOIZE => self.load_memoize(),
            FRAME => self.load_frame(),
            BYTEARRAY8 => self.load_bytearray8(),
            NEXT_BUFFER => self.load_next_buffer(),
            READONLY_BUFFER => self.load_readonly_buffer(),
            _ => unreachable!("Unspported op: {}", op as i32),
        }
    }

    fn load_mark(&mut self) -> Result<()> {
        self.metastack.push(self.stack.clone());
        self.stack.clear();
        Ok(())
    }

    fn load_stop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    fn load_pop(&mut self) -> Result<()> {
        if self.stack.len() > 0 {
            self.stack.pop();
        } else {
            self.pop_mark();
        }
        Ok(())
    }

    fn load_pop_mark(&mut self) -> Result<()> {
        self.pop_mark();
        Ok(())
    }

    fn load_dup(&mut self) -> Result<()> {
        let value = self
            .stack
            .last()
            .ok_or_else(|| Error::Syntax(ErrorCode::StackUnderflow))?
            .clone();
        self.stack.push(value);
        Ok(())
    }

    fn load_float(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = String::new();
        file.read_line(&mut buf)?;
        let f = buf.trim().parse::<f64>()?;
        self.stack.push(Value::F64(f));
        Ok(())
    }

    fn load_int(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = String::new();
        file.read_line(&mut buf)?;

        if buf.as_bytes() == TRUE {
            self.stack.push(Value::Bool(true));
        } else if buf.as_bytes() == FALSE {
            self.stack.push(Value::Bool(false));
        } else {
            let i = buf.trim().parse::<i32>()?;
            self.stack.push(Value::Int(i));
        }

        Ok(())
    }

    fn load_binint(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = [0; 4];
        file.read_exact(&mut buf)?;

        let value = LittleEndian::read_i32(&buf);
        self.stack.push(Value::Int(value));
        Ok(())
    }

    fn load_binint1(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = [0; 1];
        file.read_exact(&mut buf)?;
        let value = buf[0] as i32;
        self.stack.push(Value::Int(value));
        Ok(())
    }

    fn load_long(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = String::new();
        file.read_line(&mut buf)?;

        let buf = buf.trim().trim_end_matches('L');
        println!("I64 buf: {}", buf);
        let value = buf.parse::<i128>()?;
        self.stack.push(Value::I128(value));
        Ok(())
    }

    fn load_binint2(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = [0; 2];
        file.read_exact(&mut buf)?;

        let value = LittleEndian::read_u16(&buf);
        self.stack.push(Value::Int(value as i32));
        todo!("Unpickler::load_binint2")
    }

    fn load_none(&mut self) -> Result<()> {
        self.stack.push(Value::None);
        Ok(())
    }

    fn load_persid(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = String::new();
        file.read_line(&mut buf)?;
        self.stack.push(Value::PersId(buf));
        Ok(())
    }

    fn load_binpersid(&mut self) -> Result<()> {
        let value = self
            .stack
            .pop()
            .ok_or_else(|| Error::Syntax(ErrorCode::StackUnderflow))?;

        self.stack.push(Value::BinPersId(Box::new(value)));
        Ok(())
    }

    fn load_reduce(&self) -> Result<()> {
        panic!("load_reduce: not supported")
    }

    fn load_string(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = Vec::new();
        file.read_until(b'\n', &mut buf)?;
        buf.pop()
            .ok_or_else(|| Error::Syntax(ErrorCode::InvalidString))?;

        if buf.len() > 3 && buf[0] == b'\'' && buf[0] == buf[buf.len() - 2] {
            let s = self.decode_string(buf[1..buf.len() - 2].as_ref())?;
            self.stack.push(Value::String(s));
            Ok(())
        } else {
            Err(Error::Syntax(ErrorCode::InvalidString))
        }
    }

    fn load_binstring(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = [0; 4];
        file.read_exact(&mut buf)?;

        let len = LittleEndian::read_i32(&buf) as usize;
        let mut buf = vec![0; len];
        file.read_exact(&mut buf)?;

        let s = self.decode_string(&buf)?;
        self.stack.push(Value::String(s));
        Ok(())
    }

    fn load_short_binstring(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = [0; 1];
        file.read_exact(&mut buf)?;

        let len = buf[0] as usize;
        let mut buf = vec![0; len];
        file.read_exact(&mut buf)?;

        let s = self.decode_string(&buf)?;
        self.stack.push(Value::String(s));
        Ok(())
    }

    fn load_unicode(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = Vec::new();
        file.read_until(b'\n', &mut buf)?;

        let s = String::from_utf8(buf)?;
        self.stack.push(Value::String(s));
        Ok(())
    }

    fn load_binunicode(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = [0; 4];
        file.read_exact(&mut buf)?;

        let n = LittleEndian::read_u32(&buf) as usize;
        let mut buf = vec![0; n];
        file.read_exact(&mut buf)?;

        let s = String::from_utf8(buf)?;
        self.stack.push(Value::String(s));
        Ok(())
    }

    fn load_append(&mut self) -> Result<()> {
        let value = self
            .stack
            .pop()
            .ok_or_else(|| Error::Syntax(ErrorCode::StackUnderflow))?;

        let list = self
            .stack
            .last_mut()
            .ok_or_else(|| Error::Syntax(ErrorCode::StackUnderflow))?;

        match list {
            Value::List(ref mut l) => {
                l.push(value);
                Ok(())
            }
            _ => Err(Error::Syntax(ErrorCode::InvalidValue(format!(
                "Expected list on stack but found {:?}",
                list
            )))),
        }
    }

    fn load_build(&self) -> Result<()> {
        todo!("Unpickler::load_build")
    }

    fn load_global(&self) -> Result<()> {
        // module = self.readline()[:-1].decode("utf-8")
        // name = self.readline()[:-1].decode("utf-8")
        // klass = self.find_class(module, name)
        // self.append(klass)

        todo!("Unpickler::load_global")
    }

    fn load_dict(&mut self) -> Result<()> {
        let items = self.pop_mark();
        let mut values = Vec::new();
        for i in (0..items.len()).step_by(2) {
            let key = items[i].clone();
            let value = items[i + 1].clone();
            values.push((key, value));
        }

        self.stack.push(Value::Dict(values));
        Ok(())
    }

    fn load_empty_dict(&self) -> Result<()> {
        todo!("Unpickler::load_empty_dict")
    }

    fn load_appends(&self) -> Result<()> {
        todo!("Unpickler::load_appends")
    }

    fn load_get(&self) -> Result<()> {
        todo!("Unpickler::load_get")
    }

    fn load_binget(&self) -> Result<()> {
        todo!("Unpickler::load_binget")
    }

    fn load_inst(&self) -> Result<()> {
        todo!("Unpickler::load_inst")
    }

    fn load_long_binget(&self) -> Result<()> {
        todo!("Unpickler::load_long_binget")
    }

    fn load_list(&self) -> Result<()> {
        todo!("Unpickler::load_list")
    }

    fn load_empty_list(&self) -> Result<()> {
        todo!("Unpickler::load_empty_list")
    }

    fn load_obj(&self) -> Result<()> {
        todo!("Unpickler::load_obj")
    }

    fn load_put(&mut self, mut file: impl BufRead) -> Result<()> {
        let mut buf = String::new();
        file.read_line(&mut buf)?;

        let i = buf.trim().parse::<u32>()?;
        self.memo.insert(i, self.stack.last().unwrap().clone());
        Ok(())
    }

    fn load_binput(&self) -> Result<()> {
        todo!("Unpickler::load_binput")
    }

    fn load_long_binput(&self) -> Result<()> {
        todo!("Unpickler::load_long_binput")
    }

    fn load_setitem(&mut self) -> Result<()> {
        let value = self.stack.pop().unwrap();
        let key = self.stack.pop().unwrap();
        let dict = self.stack.last_mut().unwrap();
        match dict {
            Value::Dict(dict) => {
                dict.push((key, value));
            }
            _ => unreachable!("Instead of Dict found: {:?}", dict),
        }
        Ok(())
    }

    fn load_tuple(&mut self) -> Result<()> {
        let items = self.pop_mark();
        self.stack.push(Value::Tuple(items));
        Ok(())
    }

    fn load_empty_tuple(&self) -> Result<()> {
        todo!("Unpickler::load_empty_tuple")
    }

    fn load_setitems(&mut self) -> Result<()> {
        todo!("Unpickler::load_setitems")
    }

    fn load_binfloat(&self) -> Result<()> {
        todo!("Unpickler::load_binfloat")
    }

    fn load_proto(&mut self, mut file: impl BufRead) -> Result<()> {
        // proto = self.read(1)[0]
        // if not 0 <= proto <= HIGHEST_PROTOCOL:
        //     raise ValueError("unsupported pickle protocol: %d" % proto)
        // self.proto = proto
        const HIGHEST_PROTOCOL: u8 = 5;

        let mut buf = [0; 1];
        file.read_exact(&mut buf)?;
        let proto = buf[0];

        if (0..=HIGHEST_PROTOCOL).contains(&proto) {
            self.proto = proto;
        } else {
            return Err(Error::Syntax(ErrorCode::InvalidValue(
                "unsupported pickle protocol".to_string(),
            )));
        }
        Ok(())
    }

    fn load_newobj(&self) -> Result<()> {
        todo!("Unpickler::load_newobj")
    }

    fn load_ext1(&self) -> Result<()> {
        todo!("Unpickler::load_ext1")
    }

    fn load_ext2(&self) -> Result<()> {
        todo!("Unpickler::load_ext2")
    }

    fn load_ext4(&self) -> Result<()> {
        todo!("Unpickler::load_ext4")
    }

    fn load_tuple1(&self) -> Result<()> {
        todo!("Unpickler::load_tuple1")
    }

    fn load_tuple2(&self) -> Result<()> {
        todo!("Unpickler::load_tuple2")
    }

    fn load_tuple3(&self) -> Result<()> {
        todo!("Unpickler::load_tuple3")
    }

    fn load_newtrue(&self) -> Result<()> {
        todo!("Unpickler::load_newtrue")
    }

    fn load_newfalse(&self) -> Result<()> {
        todo!("Unpickler::load_newfalse")
    }

    fn load_long1(&self) -> Result<()> {
        todo!("Unpickler::load_long1")
    }

    fn load_long4(&self) -> Result<()> {
        todo!("Unpickler::load_long4")
    }

    fn load_binbytes(&self) -> Result<()> {
        todo!("Unpickler::load_binbytes")
    }

    fn load_short_binbytes(&self) -> Result<()> {
        todo!("Unpickler::load_short_binbytes")
    }

    fn load_short_binunicode(&self) -> Result<()> {
        todo!("Unpickler::load_short_binunicode")
    }

    fn load_binunicode8(&self) -> Result<()> {
        todo!("Unpickler::load_binunicode8")
    }

    fn load_binbytes8(&self) -> Result<()> {
        todo!("Unpickler::load_binbytes8")
    }

    fn load_empty_set(&self) -> Result<()> {
        todo!("Unpickler::load_empty_set")
    }

    fn load_additems(&self) -> Result<()> {
        todo!("Unpickler::load_additems")
    }

    fn load_frozenset(&self) -> Result<()> {
        todo!("Unpickler::load_frozenset")
    }

    fn load_newobj_ex(&self) -> Result<()> {
        todo!("Unpickler::load_newobj_ex")
    }

    fn load_stack_global(&self) -> Result<()> {
        todo!("Unpickler::load_stack_global")
    }

    fn load_memoize(&self) -> Result<()> {
        todo!("Unpickler::load_memoize")
    }

    fn load_frame(&self) -> Result<()> {
        todo!("Unpickler::load_frame")
    }

    fn load_bytearray8(&self) -> Result<()> {
        todo!("Unpickler::load_bytearray8")
    }

    fn load_next_buffer(&self) -> Result<()> {
        todo!("Unpickler::load_next_buffer")
    }

    fn load_readonly_buffer(&self) -> Result<()> {
        todo!("Unpickler::load_readonly_buffer")
    }

    fn pop_mark(&mut self) -> Vec<Value> {
        let items = self.stack.clone();
        self.stack = self.metastack.pop().unwrap();
        items
    }

    fn decode_string(&self, _data: &[u8]) -> Result<String> {
        todo!("Unpickler::decode_string")
    }
}

// class _Unpickler:

//     def __init__(self, file, *, fix_imports=True,
//                  encoding="ASCII", errors="strict", buffers=None):
//         """This takes a binary file for reading a pickle data stream.

//         The protocol version of the pickle is detected automatically, so
//         no proto argument is needed.

//         The argument *file* must have two methods, a read() method that
//         takes an integer argument, and a readline() method that requires
//         no arguments.  Both methods should return bytes.  Thus *file*
//         can be a binary file object opened for reading, an io.BytesIO
//         object, or any other custom object that meets this interface.

//         The file-like object must have two methods, a read() method
//         that takes an integer argument, and a readline() method that
//         requires no arguments.  Both methods should return bytes.
//         Thus file-like object can be a binary file object opened for
//         reading, a BytesIO object, or any other custom object that
//         meets this interface.

//         If *buffers* is not None, it should be an iterable of buffer-enabled
//         objects that is consumed each time the pickle stream references
//         an out-of-band buffer view.  Such buffers have been given in order
//         to the *buffer_callback* of a Pickler object.

//         If *buffers* is None (the default), then the buffers are taken
//         from the pickle stream, assuming they are serialized there.
//         It is an error for *buffers* to be None if the pickle stream
//         was produced with a non-None *buffer_callback*.

//         Other optional arguments are *fix_imports*, *encoding* and
//         *errors*, which are used to control compatibility support for
//         pickle stream generated by Python 2.  If *fix_imports* is True,
//         pickle will try to map the old Python 2 names to the new names
//         used in Python 3.  The *encoding* and *errors* tell pickle how
//         to decode 8-bit string instances pickled by Python 2; these
//         default to 'ASCII' and 'strict', respectively. *encoding* can be
//         'bytes' to read these 8-bit string instances as bytes objects.
//         """
//         self._buffers = iter(buffers) if buffers is not None else None
//         self._file_readline = file.readline
//         self._file_read = file.read
//         self.memo = {}
//         self.encoding = encoding
//         self.errors = errors
//         self.proto = 0
//         self.fix_imports = fix_imports

//     def load(self):
//         """Read a pickled object representation from the open file.

//         Return the reconstituted object hierarchy specified in the file.
//         """
//         # Check whether Unpickler was initialized correctly. This is
//         # only needed to mimic the behavior of _pickle.Unpickler.dump().
//         if not hasattr(self, "_file_read"):
//             raise UnpicklingError("Unpickler.__init__() was not called by "
//                                   "%s.__init__()" % (self.__class__.__name__,))
//         self._unframer = _Unframer(self._file_read, self._file_readline)
//         self.read = self._unframer.read
//         self.readinto = self._unframer.readinto
//         self.readline = self._unframer.readline
//         self.metastack = []
//         self.stack = []
//         self.append = self.stack.append
//         self.proto = 0
//         read = self.read
//         dispatch = self.dispatch
//         try:
//             while True:
//                 key = read(1)
//                 if not key:
//                     raise EOFError
//                 assert isinstance(key, bytes_types)
//                 dispatch[key[0]](self)
//         except _Stop as stopinst:
//             return stopinst.value

//     # Return a list of items pushed in the stack after last MARK instruction.
//     def pop_mark(self):
//         items = self.stack
//         self.stack = self.metastack.pop()
//         self.append = self.stack.append
//         return items

//     def persistent_load(self, pid):
//         raise UnpicklingError("unsupported persistent id encountered")

//     dispatch = {}

//     def load_proto(self):
//         proto = self.read(1)[0]
//         if not 0 <= proto <= HIGHEST_PROTOCOL:
//             raise ValueError("unsupported pickle protocol: %d" % proto)
//         self.proto = proto
//     dispatch[PROTO[0]] = load_proto

//     def load_frame(self):
//         frame_size, = unpack('<Q', self.read(8))
//         if frame_size > sys.maxsize:
//             raise ValueError("frame size > sys.maxsize: %d" % frame_size)
//         self._unframer.load_frame(frame_size)
//     dispatch[FRAME[0]] = load_frame

//     def load_persid(self):
//         try:
//             pid = self.readline()[:-1].decode("ascii")
//         except UnicodeDecodeError:
//             raise UnpicklingError(
//                 "persistent IDs in protocol 0 must be ASCII strings")
//         self.append(self.persistent_load(pid))
//     dispatch[PERSID[0]] = load_persid

//     def load_binpersid(self):
//         pid = self.stack.pop()
//         self.append(self.persistent_load(pid))
//     dispatch[BINPERSID[0]] = load_binpersid

//     def load_none(self):
//         self.append(None)
//     dispatch[NONE[0]] = load_none

//     def load_false(self):
//         self.append(False)
//     dispatch[NEWFALSE[0]] = load_false

//     def load_true(self):
//         self.append(True)
//     dispatch[NEWTRUE[0]] = load_true

//     def load_int(self):
//         data = self.readline()
//         if data == FALSE[1:]:
//             val = False
//         elif data == TRUE[1:]:
//             val = True
//         else:
//             val = int(data, 0)
//         self.append(val)
//     dispatch[INT[0]] = load_int

//     def load_binint(self):
//         self.append(unpack('<i', self.read(4))[0])
//     dispatch[BININT[0]] = load_binint

//     def load_binint1(self):
//         self.append(self.read(1)[0])
//     dispatch[BININT1[0]] = load_binint1

//     def load_binint2(self):
//         self.append(unpack('<H', self.read(2))[0])
//     dispatch[BININT2[0]] = load_binint2

//     def load_long(self):
//         val = self.readline()[:-1]
//         if val and val[-1] == b'L'[0]:
//             val = val[:-1]
//         self.append(int(val, 0))
//     dispatch[LONG[0]] = load_long

//     def load_long1(self):
//         n = self.read(1)[0]
//         data = self.read(n)
//         self.append(decode_long(data))
//     dispatch[LONG1[0]] = load_long1

//     def load_long4(self):
//         n, = unpack('<i', self.read(4))
//         if n < 0:
//             # Corrupt or hostile pickle -- we never write one like this
//             raise UnpicklingError("LONG pickle has negative byte count")
//         data = self.read(n)
//         self.append(decode_long(data))
//     dispatch[LONG4[0]] = load_long4

//     def load_float(self):
//         self.append(float(self.readline()[:-1]))
//     dispatch[FLOAT[0]] = load_float

//     def load_binfloat(self):
//         self.append(unpack('>d', self.read(8))[0])
//     dispatch[BINFLOAT[0]] = load_binfloat

//     def _decode_string(self, value):
//         # Used to allow strings from Python 2 to be decoded either as
//         # bytes or Unicode strings.  This should be used only with the
//         # STRING, BINSTRING and SHORT_BINSTRING opcodes.
//         if self.encoding == "bytes":
//             return value
//         else:
//             return value.decode(self.encoding, self.errors)

//     def load_string(self):
//         data = self.readline()[:-1]
//         # Strip outermost quotes
//         if len(data) >= 2 and data[0] == data[-1] and data[0] in b'"\'':
//             data = data[1:-1]
//         else:
//             raise UnpicklingError("the STRING opcode argument must be quoted")
//         self.append(self._decode_string(codecs.escape_decode(data)[0]))
//     dispatch[STRING[0]] = load_string

//     def load_binstring(self):
//         # Deprecated BINSTRING uses signed 32-bit length
//         len, = unpack('<i', self.read(4))
//         if len < 0:
//             raise UnpicklingError("BINSTRING pickle has negative byte count")
//         data = self.read(len)
//         self.append(self._decode_string(data))
//     dispatch[BINSTRING[0]] = load_binstring

//     def load_binbytes(self):
//         len, = unpack('<I', self.read(4))
//         if len > maxsize:
//             raise UnpicklingError("BINBYTES exceeds system's maximum size "
//                                   "of %d bytes" % maxsize)
//         self.append(self.read(len))
//     dispatch[BINBYTES[0]] = load_binbytes

//     def load_unicode(self):
//         self.append(str(self.readline()[:-1], 'raw-unicode-escape'))
//     dispatch[UNICODE[0]] = load_unicode

//     def load_binunicode(self):
//         len, = unpack('<I', self.read(4))
//         if len > maxsize:
//             raise UnpicklingError("BINUNICODE exceeds system's maximum size "
//                                   "of %d bytes" % maxsize)
//         self.append(str(self.read(len), 'utf-8', 'surrogatepass'))
//     dispatch[BINUNICODE[0]] = load_binunicode

//     def load_binunicode8(self):
//         len, = unpack('<Q', self.read(8))
//         if len > maxsize:
//             raise UnpicklingError("BINUNICODE8 exceeds system's maximum size "
//                                   "of %d bytes" % maxsize)
//         self.append(str(self.read(len), 'utf-8', 'surrogatepass'))
//     dispatch[BINUNICODE8[0]] = load_binunicode8

//     def load_binbytes8(self):
//         len, = unpack('<Q', self.read(8))
//         if len > maxsize:
//             raise UnpicklingError("BINBYTES8 exceeds system's maximum size "
//                                   "of %d bytes" % maxsize)
//         self.append(self.read(len))
//     dispatch[BINBYTES8[0]] = load_binbytes8

//     def load_bytearray8(self):
//         len, = unpack('<Q', self.read(8))
//         if len > maxsize:
//             raise UnpicklingError("BYTEARRAY8 exceeds system's maximum size "
//                                   "of %d bytes" % maxsize)
//         b = bytearray(len)
//         self.readinto(b)
//         self.append(b)
//     dispatch[BYTEARRAY8[0]] = load_bytearray8

//     def load_next_buffer(self):
//         if self._buffers is None:
//             raise UnpicklingError("pickle stream refers to out-of-band data "
//                                   "but no *buffers* argument was given")
//         try:
//             buf = next(self._buffers)
//         except StopIteration:
//             raise UnpicklingError("not enough out-of-band buffers")
//         self.append(buf)
//     dispatch[NEXT_BUFFER[0]] = load_next_buffer

//     def load_readonly_buffer(self):
//         buf = self.stack[-1]
//         with memoryview(buf) as m:
//             if not m.readonly:
//                 self.stack[-1] = m.toreadonly()
//     dispatch[READONLY_BUFFER[0]] = load_readonly_buffer

//     def load_short_binstring(self):
//         len = self.read(1)[0]
//         data = self.read(len)
//         self.append(self._decode_string(data))
//     dispatch[SHORT_BINSTRING[0]] = load_short_binstring

//     def load_short_binbytes(self):
//         len = self.read(1)[0]
//         self.append(self.read(len))
//     dispatch[SHORT_BINBYTES[0]] = load_short_binbytes

//     def load_short_binunicode(self):
//         len = self.read(1)[0]
//         self.append(str(self.read(len), 'utf-8', 'surrogatepass'))
//     dispatch[SHORT_BINUNICODE[0]] = load_short_binunicode

//     def load_tuple(self):
//         items = self.pop_mark()
//         self.append(tuple(items))
//     dispatch[TUPLE[0]] = load_tuple

//     def load_empty_tuple(self):
//         self.append(())
//     dispatch[EMPTY_TUPLE[0]] = load_empty_tuple

//     def load_tuple1(self):
//         self.stack[-1] = (self.stack[-1],)
//     dispatch[TUPLE1[0]] = load_tuple1

//     def load_tuple2(self):
//         self.stack[-2:] = [(self.stack[-2], self.stack[-1])]
//     dispatch[TUPLE2[0]] = load_tuple2

//     def load_tuple3(self):
//         self.stack[-3:] = [(self.stack[-3], self.stack[-2], self.stack[-1])]
//     dispatch[TUPLE3[0]] = load_tuple3

//     def load_empty_list(self):
//         self.append([])
//     dispatch[EMPTY_LIST[0]] = load_empty_list

//     def load_empty_dictionary(self):
//         self.append({})
//     dispatch[EMPTY_DICT[0]] = load_empty_dictionary

//     def load_empty_set(self):
//         self.append(set())
//     dispatch[EMPTY_SET[0]] = load_empty_set

//     def load_frozenset(self):
//         items = self.pop_mark()
//         self.append(frozenset(items))
//     dispatch[FROZENSET[0]] = load_frozenset

//     def load_list(self):
//         items = self.pop_mark()
//         self.append(items)
//     dispatch[LIST[0]] = load_list

//     def load_dict(self):
//         items = self.pop_mark()
//         d = {items[i]: items[i+1]
//              for i in range(0, len(items), 2)}
//         self.append(d)
//     dispatch[DICT[0]] = load_dict

//     # INST and OBJ differ only in how they get a class object.  It's not
//     # only sensible to do the rest in a common routine, the two routines
//     # previously diverged and grew different bugs.
//     # klass is the class to instantiate, and k points to the topmost mark
//     # object, following which are the arguments for klass.__init__.
//     def _instantiate(self, klass, args):
//         if (args or not isinstance(klass, type) or
//             hasattr(klass, "__getinitargs__")):
//             try:
//                 value = klass(*args)
//             except TypeError as err:
//                 raise TypeError("in constructor for %s: %s" %
//                                 (klass.__name__, str(err)), err.__traceback__)
//         else:
//             value = klass.__new__(klass)
//         self.append(value)

//     def load_inst(self):
//         module = self.readline()[:-1].decode("ascii")
//         name = self.readline()[:-1].decode("ascii")
//         klass = self.find_class(module, name)
//         self._instantiate(klass, self.pop_mark())
//     dispatch[INST[0]] = load_inst

//     def load_obj(self):
//         # Stack is ... markobject classobject arg1 arg2 ...
//         args = self.pop_mark()
//         cls = args.pop(0)
//         self._instantiate(cls, args)
//     dispatch[OBJ[0]] = load_obj

//     def load_newobj(self):
//         args = self.stack.pop()
//         cls = self.stack.pop()
//         obj = cls.__new__(cls, *args)
//         self.append(obj)
//     dispatch[NEWOBJ[0]] = load_newobj

//     def load_newobj_ex(self):
//         kwargs = self.stack.pop()
//         args = self.stack.pop()
//         cls = self.stack.pop()
//         obj = cls.__new__(cls, *args, **kwargs)
//         self.append(obj)
//     dispatch[NEWOBJ_EX[0]] = load_newobj_ex

//     def load_global(self):
//         module = self.readline()[:-1].decode("utf-8")
//         name = self.readline()[:-1].decode("utf-8")
//         klass = self.find_class(module, name)
//         self.append(klass)
//     dispatch[GLOBAL[0]] = load_global

//     def load_stack_global(self):
//         name = self.stack.pop()
//         module = self.stack.pop()
//         if type(name) is not str or type(module) is not str:
//             raise UnpicklingError("STACK_GLOBAL requires str")
//         self.append(self.find_class(module, name))
//     dispatch[STACK_GLOBAL[0]] = load_stack_global

//     def load_ext1(self):
//         code = self.read(1)[0]
//         self.get_extension(code)
//     dispatch[EXT1[0]] = load_ext1

//     def load_ext2(self):
//         code, = unpack('<H', self.read(2))
//         self.get_extension(code)
//     dispatch[EXT2[0]] = load_ext2

//     def load_ext4(self):
//         code, = unpack('<i', self.read(4))
//         self.get_extension(code)
//     dispatch[EXT4[0]] = load_ext4

//     def get_extension(self, code):
//         nil = []
//         obj = _extension_cache.get(code, nil)
//         if obj is not nil:
//             self.append(obj)
//             return
//         key = _inverted_registry.get(code)
//         if not key:
//             if code <= 0: # note that 0 is forbidden
//                 # Corrupt or hostile pickle.
//                 raise UnpicklingError("EXT specifies code <= 0")
//             raise ValueError("unregistered extension code %d" % code)
//         obj = self.find_class(*key)
//         _extension_cache[code] = obj
//         self.append(obj)

//     def find_class(self, module, name):
//         # Subclasses may override this.
//         sys.audit('pickle.find_class', module, name)
//         if self.proto < 3 and self.fix_imports:
//             if (module, name) in _compat_pickle.NAME_MAPPING:
//                 module, name = _compat_pickle.NAME_MAPPING[(module, name)]
//             elif module in _compat_pickle.IMPORT_MAPPING:
//                 module = _compat_pickle.IMPORT_MAPPING[module]
//         __import__(module, level=0)
//         if self.proto >= 4:
//             return _getattribute(sys.modules[module], name)[0]
//         else:
//             return getattr(sys.modules[module], name)

//     def load_reduce(self):
//         stack = self.stack
//         args = stack.pop()
//         func = stack[-1]
//         stack[-1] = func(*args)
//     dispatch[REDUCE[0]] = load_reduce

//     def load_pop(self):
//         if self.stack:
//             del self.stack[-1]
//         else:
//             self.pop_mark()
//     dispatch[POP[0]] = load_pop

//     def load_pop_mark(self):
//         self.pop_mark()
//     dispatch[POP_MARK[0]] = load_pop_mark

//     def load_dup(self):
//         self.append(self.stack[-1])
//     dispatch[DUP[0]] = load_dup

//     def load_get(self):
//         i = int(self.readline()[:-1])
//         try:
//             self.append(self.memo[i])
//         except KeyError:
//             msg = f'Memo value not found at index {i}'
//             raise UnpicklingError(msg) from None
//     dispatch[GET[0]] = load_get

//     def load_binget(self):
//         i = self.read(1)[0]
//         try:
//             self.append(self.memo[i])
//         except KeyError as exc:
//             msg = f'Memo value not found at index {i}'
//             raise UnpicklingError(msg) from None
//     dispatch[BINGET[0]] = load_binget

//     def load_long_binget(self):
//         i, = unpack('<I', self.read(4))
//         try:
//             self.append(self.memo[i])
//         except KeyError as exc:
//             msg = f'Memo value not found at index {i}'
//             raise UnpicklingError(msg) from None
//     dispatch[LONG_BINGET[0]] = load_long_binget

//     def load_put(self):
//         i = int(self.readline()[:-1])
//         if i < 0:
//             raise ValueError("negative PUT argument")
//         self.memo[i] = self.stack[-1]
//     dispatch[PUT[0]] = load_put

//     def load_binput(self):
//         i = self.read(1)[0]
//         if i < 0:
//             raise ValueError("negative BINPUT argument")
//         self.memo[i] = self.stack[-1]
//     dispatch[BINPUT[0]] = load_binput

//     def load_long_binput(self):
//         i, = unpack('<I', self.read(4))
//         if i > maxsize:
//             raise ValueError("negative LONG_BINPUT argument")
//         self.memo[i] = self.stack[-1]
//     dispatch[LONG_BINPUT[0]] = load_long_binput

//     def load_memoize(self):
//         memo = self.memo
//         memo[len(memo)] = self.stack[-1]
//     dispatch[MEMOIZE[0]] = load_memoize

//     def load_append(self):
//         stack = self.stack
//         value = stack.pop()
//         list = stack[-1]
//         list.append(value)
//     dispatch[APPEND[0]] = load_append

//     def load_appends(self):
//         items = self.pop_mark()
//         list_obj = self.stack[-1]
//         try:
//             extend = list_obj.extend
//         except AttributeError:
//             pass
//         else:
//             extend(items)
//             return
//         # Even if the PEP 307 requires extend() and append() methods,
//         # fall back on append() if the object has no extend() method
//         # for backward compatibility.
//         append = list_obj.append
//         for item in items:
//             append(item)
//     dispatch[APPENDS[0]] = load_appends

//     def load_setitem(self):
//         stack = self.stack
//         value = stack.pop()
//         key = stack.pop()
//         dict = stack[-1]
//         dict[key] = value
//     dispatch[SETITEM[0]] = load_setitem

//     def load_setitems(self):
//         items = self.pop_mark()
//         dict = self.stack[-1]
//         for i in range(0, len(items), 2):
//             dict[items[i]] = items[i + 1]
//     dispatch[SETITEMS[0]] = load_setitems

//     def load_additems(self):
//         items = self.pop_mark()
//         set_obj = self.stack[-1]
//         if isinstance(set_obj, set):
//             set_obj.update(items)
//         else:
//             add = set_obj.add
//             for item in items:
//                 add(item)
//     dispatch[ADDITEMS[0]] = load_additems

//     def load_build(self):
//         stack = self.stack
//         state = stack.pop()
//         inst = stack[-1]
//         setstate = getattr(inst, "__setstate__", _NoValue)
//         if setstate is not _NoValue:
//             setstate(state)
//             return
//         slotstate = None
//         if isinstance(state, tuple) and len(state) == 2:
//             state, slotstate = state
//         if state:
//             inst_dict = inst.__dict__
//             intern = sys.intern
//             for k, v in state.items():
//                 if type(k) is str:
//                     inst_dict[intern(k)] = v
//                 else:
//                     inst_dict[k] = v
//         if slotstate:
//             for k, v in slotstate.items():
//                 setattr(inst, k, v)
//     dispatch[BUILD[0]] = load_build

//     def load_mark(self):
//         self.metastack.append(self.stack)
//         self.stack = []
//         self.append = self.stack.append
//     dispatch[MARK[0]] = load_mark

//     def load_stop(self):
//         value = self.stack.pop()
//         raise _Stop(value)
//     dispatch[STOP[0]] = load_stop

// # Shorthands

// def _dump(obj, file, protocol=None, *, fix_imports=True, buffer_callback=None):
//     _Pickler(file, protocol, fix_imports=fix_imports,
//              buffer_callback=buffer_callback).dump(obj)

// def _dumps(obj, protocol=None, *, fix_imports=True, buffer_callback=None):
//     f = io.BytesIO()
//     _Pickler(f, protocol, fix_imports=fix_imports,
//              buffer_callback=buffer_callback).dump(obj)
//     res = f.getvalue()
//     assert isinstance(res, bytes_types)
//     return res

// def _load(file, *, fix_imports=True, encoding="ASCII", errors="strict",
//           buffers=None):
//     return _Unpickler(file, fix_imports=fix_imports, buffers=buffers,
//                      encoding=encoding, errors=errors).load()

// def _loads(s, /, *, fix_imports=True, encoding="ASCII", errors="strict",
//            buffers=None):
//     if isinstance(s, str):
//         raise TypeError("Can't load pickle from unicode string")
//     file = io.BytesIO(s)
//     return _Unpickler(file, fix_imports=fix_imports, buffers=buffers,
//                       encoding=encoding, errors=errors).load()

// # Use the faster _pickle if possible
// try:
//     from _pickle import (
//         PickleError,
//         PicklingError,
//         UnpicklingError,
//         Pickler,
//         Unpickler,
//         dump,
//         dumps,
//         load,
//         loads
//     )
// except ImportError:
//     Pickler, Unpickler = _Pickler, _Unpickler
//     dump, dumps, load, loads = _dump, _dumps, _load, _loads

// # Doctest
// def _test():
//     import doctest
//     return doctest.testmod()

// if __name__ == "__main__":
//     import argparse
//     parser = argparse.ArgumentParser(
//         description='display contents of the pickle files')
//     parser.add_argument(
//         'pickle_file', type=argparse.FileType('br'),
//         nargs='*', help='the pickle file')
//     parser.add_argument(
//         '-t', '--test', action='store_true',
//         help='run self-test suite')
//     parser.add_argument(
//         '-v', action='store_true',
//         help='run verbosely; only affects self-test run')
//     args = parser.parse_args()
//     if args.test:
//         _test()
//     else:
//         if not args.pickle_file:
//             parser.print_help()
//         else:
//             import pprint
//             for f in args.pickle_file:
//                 obj = load(f)
//                 pprint.pprint(obj)

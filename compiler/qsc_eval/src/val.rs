// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use num_bigint::BigInt;
use qsc_data_structures::{display::join, functors::FunctorApp};
use qsc_fir::fir::{Functor, Pauli, StoreItemId};
use std::{
    fmt::{self, Display, Formatter},
    rc::Rc,
};

use crate::{error::PackageSpan, AsIndex, Error, Range as EvalRange};

pub(super) const DEFAULT_RANGE_STEP: i64 = 1;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Array(Rc<Vec<Value>>),
    BigInt(BigInt),
    Bool(bool),
    Closure(Box<Closure>),
    Double(f64),
    Global(StoreItemId, FunctorApp),
    Int(i64),
    Pauli(Pauli),
    Qubit(Qubit),
    Range(Box<Range>),
    Result(Result),
    String(Rc<str>),
    Tuple(Rc<[Value]>),
    Var(Var),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Closure {
    pub fixed_args: Rc<[Value]>,
    pub id: StoreItemId,
    pub functor: FunctorApp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Range {
    pub start: Option<i64>,
    pub step: i64,
    pub end: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Result {
    Val(bool),
    Id(usize),
}

impl Result {
    /// Convert the `Result` into a bool
    /// # Panics
    /// This will panic if the `Result` is not a `Result::Val`.
    #[must_use]
    pub fn unwrap_bool(self) -> bool {
        match self {
            Self::Val(v) => v,
            Self::Id(_) => panic!("cannot unwrap Result::Id as bool"),
        }
    }

    /// Convert the `Result` into an id
    /// # Panics
    /// This will panic if the `Result` is not a `Result::Id`.
    #[must_use]
    pub fn unwrap_id(self) -> usize {
        match self {
            Self::Val(_) => panic!("cannot unwrap Result::Val as id"),
            Self::Id(v) => v,
        }
    }
}

impl From<bool> for Result {
    fn from(val: bool) -> Self {
        Self::Val(val)
    }
}

impl From<usize> for Result {
    fn from(val: usize) -> Self {
        Self::Id(val)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Qubit(pub usize);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Var {
    pub id: usize,
    pub ty: VarTy,
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Value::Array(arr) => {
                write!(f, "[")?;
                join(f, arr.iter(), ", ")?;
                write!(f, "]")
            }
            Value::BigInt(v) => write!(f, "{v}"),
            Value::Bool(v) => write!(f, "{v}"),
            Value::Closure(..) => f.write_str("<closure>"),
            Value::Double(v) => {
                if (v.floor() - v.ceil()).abs() < f64::EPSILON {
                    // The value is a whole number, which by convention is displayed with one decimal point
                    // to differentiate it from an integer value.
                    write!(f, "{v:.1}")
                } else {
                    write!(f, "{v}")
                }
            }
            Value::Global(id, functor) if functor == &FunctorApp::default() => id.fmt(f),
            Value::Global(id, functor) => write!(f, "{functor} {id}"),
            Value::Int(v) => write!(f, "{v}"),
            Value::Pauli(v) => match v {
                Pauli::I => write!(f, "PauliI"),
                Pauli::X => write!(f, "PauliX"),
                Pauli::Z => write!(f, "PauliZ"),
                Pauli::Y => write!(f, "PauliY"),
            },
            Value::Qubit(v) => write!(f, "Qubit{}", (v.0)),
            Value::Range(inner) => match (inner.start, inner.step, inner.end) {
                (Some(start), DEFAULT_RANGE_STEP, Some(end)) => write!(f, "{start}..{end}"),
                (Some(start), DEFAULT_RANGE_STEP, None) => write!(f, "{start}..."),
                (Some(start), step, Some(end)) => write!(f, "{start}..{step}..{end}"),
                (Some(start), step, None) => write!(f, "{start}..{step}..."),
                (None, DEFAULT_RANGE_STEP, Some(end)) => write!(f, "...{end}"),
                (None, DEFAULT_RANGE_STEP, None) => write!(f, "..."),
                (None, step, Some(end)) => write!(f, "...{step}..{end}"),
                (None, step, None) => write!(f, "...{step}..."),
            },
            Value::Result(v) => match v {
                Result::Id(id) => write!(f, "Result({id})"),
                Result::Val(val) => {
                    if *val {
                        write!(f, "One")
                    } else {
                        write!(f, "Zero")
                    }
                }
            },
            Value::String(v) => write!(f, "{v}"),
            Value::Tuple(tup) => {
                write!(f, "(")?;
                join(f, tup.iter(), ", ")?;
                if tup.len() == 1 {
                    write!(f, ",")?;
                }
                write!(f, ")")
            }
            Value::Var(var) => write!(f, "Var({}, {})", var.id, var.ty),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VarTy {
    Boolean,
    Integer,
    Double,
}

impl Display for VarTy {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Boolean => write!(f, "Boolean"),
            Self::Integer => write!(f, "Integer"),
            Self::Double => write!(f, "Double"),
        }
    }
}

thread_local! {
    static UNIT: Rc<[Value; 0]> = Rc::new([]);
}

impl Value {
    pub const RESULT_ZERO: Self = Self::Result(Result::Val(false));
    pub const RESULT_ONE: Self = Self::Result(Result::Val(true));

    #[must_use]
    pub fn unit() -> Self {
        UNIT.with(|unit| Self::Tuple(unit.clone()))
    }

    /// Convert the [Value] into an array of [Value]
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Array`].
    #[must_use]
    pub fn unwrap_array(self) -> Rc<Vec<Self>> {
        let Value::Array(v) = self else {
            panic!("value should be Array, got {}", self.type_name());
        };
        v
    }

    /// Updates a value in an array in-place.
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Array`].
    pub fn update_array(&mut self, index: usize, value: Self) -> core::result::Result<(), usize> {
        let Value::Array(arr) = self else {
            panic!("value should be Array, got {}", self.type_name());
        };
        let arr = Rc::get_mut(arr).expect("array should be uniquely referenced");
        match arr.get_mut(index) {
            Some(v) => {
                *v = value;
                Ok(())
            }
            None => Err(index),
        }
    }

    /// Appends a value to an array in-place.
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Array`].
    pub fn append_array(&mut self, value: Self) {
        let Value::Array(arr) = self else {
            panic!("value should be Array, got {}", self.type_name());
        };
        let arr = Rc::get_mut(arr).expect("array should be uniquely referenced");
        let append_arr = value.unwrap_array();
        arr.extend_from_slice(&append_arr);
    }

    /// Convert the [Value] into a `BigInt`
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::BigInt`].
    #[must_use]
    pub fn unwrap_big_int(self) -> BigInt {
        let Value::BigInt(v) = self else {
            panic!("value should be BigInt, got {}", self.type_name());
        };
        v
    }

    /// Convert the [Value] into a bool
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Bool`].
    #[must_use]
    pub fn unwrap_bool(self) -> bool {
        let Value::Bool(v) = self else {
            panic!("value should be Bool, got {}", self.type_name());
        };
        v
    }

    /// Convert the [Value] into a double
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Double`].
    #[must_use]
    pub fn unwrap_double(self) -> f64 {
        let Value::Double(v) = self else {
            panic!("value should be Double, got {}", self.type_name());
        };
        v
    }

    /// Convert the [Value] into a global tuple
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Global`].
    #[must_use]
    pub fn unwrap_global(self) -> (StoreItemId, FunctorApp) {
        let Value::Global(id, functor) = self else {
            panic!("value should be Global, got {}", self.type_name());
        };
        (id, functor)
    }

    /// Convert the [Value] into an integer
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Int`].
    #[must_use]
    pub fn unwrap_int(self) -> i64 {
        let Value::Int(v) = self else {
            panic!("value should be Int, got {}", self.type_name());
        };
        v
    }

    /// Convert the [Value] into a Pauli
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Pauli`].
    #[must_use]
    pub fn unwrap_pauli(self) -> Pauli {
        let Value::Pauli(v) = self else {
            panic!("value should be Pauli, got {}", self.type_name());
        };
        v
    }

    /// Convert the [Value] into a qubit
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Qubit`].
    #[must_use]
    pub fn unwrap_qubit(self) -> Qubit {
        let Value::Qubit(v) = self else {
            panic!("value should be Qubit, got {}", self.type_name());
        };
        v
    }

    /// Convert the [Value] into a range tuple
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Range`].
    #[must_use]
    pub fn unwrap_range(self) -> (Option<i64>, i64, Option<i64>) {
        let Value::Range(inner) = self else {
            panic!("value should be Range, got {}", self.type_name());
        };
        (inner.start, inner.step, inner.end)
    }

    /// Convert the [Value] into a measurement result
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Result`].
    #[must_use]
    pub fn unwrap_result(self) -> bool {
        let Value::Result(v) = self else {
            panic!("value should be Result, got {}", self.type_name());
        };
        v.unwrap_bool()
    }

    /// Convert the [Value] into a string
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::String`].
    #[must_use]
    pub fn unwrap_string(self) -> Rc<str> {
        let Value::String(v) = self else {
            panic!("value should be String, got {}", self.type_name());
        };
        v
    }

    /// Convert the [Value] into an array of [Value]
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Tuple`].
    #[must_use]
    pub fn unwrap_tuple(self) -> Rc<[Self]> {
        let Value::Tuple(v) = self else {
            panic!("value should be Tuple, got {}", self.type_name());
        };
        v
    }

    /// Convert the [Value] into a var
    /// # Panics
    /// This will panic if the [Value] is not a [`Value::Var`].
    #[must_use]
    pub fn unwrap_var(self) -> Var {
        let Value::Var(v) = self else {
            panic!("value should be Var, got {}", self.type_name());
        };
        v
    }

    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Array(_) => "Array",
            Value::BigInt(_) => "BigInt",
            Value::Bool(_) => "Bool",
            Value::Closure(..) => "Closure",
            Value::Double(_) => "Double",
            Value::Global(..) => "Global",
            Value::Int(_) => "Int",
            Value::Pauli(_) => "Pauli",
            Value::Qubit(_) => "Qubit",
            Value::Range(..) => "Range",
            Value::Result(_) => "Result",
            Value::String(_) => "String",
            Value::Tuple(_) => "Tuple",
            Value::Var(_) => "Var",
        }
    }

    /// Returns any qubits contained in the value as a vector. This does not
    /// consume the value, and will recursively search through any nested values.
    #[must_use]
    pub fn qubits(&self) -> Vec<Qubit> {
        match self {
            Value::Array(arr) => arr.iter().flat_map(Value::qubits).collect(),
            Value::Closure(closure) => closure.fixed_args.iter().flat_map(Value::qubits).collect(),
            Value::Qubit(q) => vec![*q],
            Value::Tuple(tup) => tup.iter().flat_map(Value::qubits).collect(),

            Value::BigInt(_)
            | Value::Bool(_)
            | Value::Double(_)
            | Value::Global(..)
            | Value::Int(_)
            | Value::Pauli(_)
            | Value::Range(_)
            | Value::Result(_)
            | Value::String(_)
            | Value::Var(_) => Vec::new(),
        }
    }
}

pub fn index_array(
    arr: &[Value],
    index: i64,
    span: PackageSpan,
) -> std::result::Result<Value, Error> {
    let i = index.as_index(span)?;
    match arr.get(i) {
        Some(v) => Ok(v.clone()),
        None => Err(Error::IndexOutOfRange(index, span)),
    }
}

pub fn make_range(
    arr: &[Value],
    start: Option<i64>,
    step: i64,
    end: Option<i64>,
    span: PackageSpan,
) -> std::result::Result<EvalRange, Error> {
    if step == 0 {
        Err(Error::RangeStepZero(span))
    } else {
        let len: i64 = match arr.len().try_into() {
            Ok(len) => Ok(len),
            Err(_) => Err(Error::ArrayTooLarge(span)),
        }?;
        let (start, end) = if step > 0 {
            (start.unwrap_or(0), end.unwrap_or(len - 1))
        } else {
            (start.unwrap_or(len - 1), end.unwrap_or(0))
        };
        Ok(EvalRange::new(start, step, end))
    }
}

pub fn slice_array(
    arr: &[Value],
    start: Option<i64>,
    step: i64,
    end: Option<i64>,
    span: PackageSpan,
) -> std::result::Result<Value, Error> {
    let range = make_range(arr, start, step, end, span)?;
    let mut slice = vec![];
    for i in range {
        slice.push(index_array(arr, i, span)?);
    }

    Ok(Value::Array(slice.into()))
}

pub fn update_index_single(
    values: &[Value],
    index: i64,
    update: Value,
    span: PackageSpan,
) -> std::result::Result<Value, Error> {
    if index < 0 {
        return Err(Error::InvalidNegativeInt(index, span));
    }
    let i = index.as_index(span)?;
    let mut values = values.to_vec();
    match values.get_mut(i) {
        Some(value) => {
            *value = update;
        }
        None => return Err(Error::IndexOutOfRange(index, span)),
    }
    Ok(Value::Array(values.into()))
}

pub fn update_index_range(
    values: &[Value],
    start: Option<i64>,
    step: i64,
    end: Option<i64>,
    update: Value,
    span: PackageSpan,
) -> std::result::Result<Value, Error> {
    let range = make_range(values, start, step, end, span)?;
    let mut values = values.to_vec();
    let update = update.unwrap_array();
    for (idx, update) in range.into_iter().zip(update.iter()) {
        let i = idx.as_index(span)?;
        match values.get_mut(i) {
            Some(value) => {
                *value = update.clone();
            }
            None => return Err(Error::IndexOutOfRange(idx, span)),
        }
    }
    Ok(Value::Array(values.into()))
}

#[must_use]
pub fn update_functor_app(functor: Functor, app: FunctorApp) -> FunctorApp {
    match functor {
        Functor::Adj => FunctorApp {
            adjoint: !app.adjoint,
            controlled: app.controlled,
        },
        Functor::Ctl => FunctorApp {
            adjoint: app.adjoint,
            controlled: app.controlled + 1,
        },
    }
}

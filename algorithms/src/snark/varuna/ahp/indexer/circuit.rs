// Copyright (C) 2019-2023 Aleo Systems Inc.
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use core::marker::PhantomData;

use crate::{
    polycommit::sonic_pc::LabeledPolynomial,
    snark::varuna::{ahp::matrices::MatrixEvals, matrices::MatrixArithmetization, CircuitInfo, Matrix, SNARKMode},
};
use blake2::Digest;
use hex::FromHex;
use snarkvm_fields::PrimeField;
use snarkvm_utilities::{serialize::*, SerializationError};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd, CanonicalSerialize, CanonicalDeserialize)]
pub struct CircuitId(pub [u8; 32]);

impl std::fmt::Display for CircuitId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for byte in self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl CircuitId {
    pub fn from_witness_label(witness_label: &str) -> Self {
        CircuitId(
            <[u8; 32]>::from_hex(witness_label.split('_').collect::<Vec<&str>>()[1])
                .expect("Decoding circuit_id failed"),
        )
    }
}

/// The indexed version of the constraint system.
/// This struct contains three kinds of objects:
/// 1) `index_info` is information about the index, such as the size of the
///     public input
/// 2) `{a,b,c}` are the matrices defining the R1CS instance
/// 3) `{a,b,c}_arith` are structs containing information about the arithmetized matrices
#[derive(Clone, Debug)]
pub struct Circuit<F: PrimeField, MM: SNARKMode> {
    /// Information about the indexed circuit.
    pub index_info: CircuitInfo,

    /// The A matrix for the R1CS instance
    pub a: Matrix<F>,
    /// The B matrix for the R1CS instance
    pub b: Matrix<F>,
    /// The C matrix for the R1CS instance
    pub c: Matrix<F>,

    /// Joint arithmetization of the A, B, and C matrices.
    pub a_arith: MatrixEvals<F>,
    pub b_arith: MatrixEvals<F>,
    pub c_arith: MatrixEvals<F>,

    pub(crate) _mode: PhantomData<MM>,
    pub(crate) id: CircuitId,
}

impl<F: PrimeField, MM: SNARKMode> Eq for Circuit<F, MM> {}
impl<F: PrimeField, MM: SNARKMode> PartialEq for Circuit<F, MM> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<F: PrimeField, MM: SNARKMode> Ord for Circuit<F, MM> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl<F: PrimeField, MM: SNARKMode> PartialOrd for Circuit<F, MM> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<F: PrimeField, MM: SNARKMode> Circuit<F, MM> {
    pub fn hash(
        index_info: &CircuitInfo,
        a: &Matrix<F>,
        b: &Matrix<F>,
        c: &Matrix<F>,
    ) -> Result<CircuitId, SerializationError> {
        let mut blake2 = blake2::Blake2s256::new();
        index_info.serialize_uncompressed(&mut blake2)?;
        a.serialize_uncompressed(&mut blake2)?;
        b.serialize_uncompressed(&mut blake2)?;
        c.serialize_uncompressed(&mut blake2)?;
        Ok(CircuitId(blake2.finalize().into()))
    }

    pub fn interpolate_matrix_evals(&self) -> impl Iterator<Item = LabeledPolynomial<F>> {
        let [a_arith, b_arith, c_arith]: [_; 3] = [("a", &self.a_arith), ("b", &self.b_arith), ("c", &self.c_arith)]
            .into_iter()
            .map(|(label, evals)| MatrixArithmetization::new(&self.id, label, evals))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .try_into()
            .unwrap();
        a_arith.into_iter().chain(b_arith.into_iter()).chain(c_arith.into_iter())
    }

    /// After indexing, we drop these evaluations to save space in the ProvingKey.
    pub fn prune_row_col_evals(&mut self) {
        self.a_arith.row_col = None;
        self.b_arith.row_col = None;
        self.c_arith.row_col = None;
    }
}

impl<F: PrimeField, MM: SNARKMode> CanonicalSerialize for Circuit<F, MM> {
    fn serialize_with_mode<W: Write>(&self, mut writer: W, compress: Compress) -> Result<(), SerializationError> {
        self.index_info.serialize_with_mode(&mut writer, compress)?;
        self.a.serialize_with_mode(&mut writer, compress)?;
        self.b.serialize_with_mode(&mut writer, compress)?;
        self.c.serialize_with_mode(&mut writer, compress)?;
        self.a_arith.serialize_with_mode(&mut writer, compress)?;
        self.b_arith.serialize_with_mode(&mut writer, compress)?;
        self.c_arith.serialize_with_mode(&mut writer, compress)?;
        Ok(())
    }

    fn serialized_size(&self, mode: Compress) -> usize {
        self.index_info
            .serialized_size(mode)
            .saturating_add(self.a.serialized_size(mode))
            .saturating_add(self.b.serialized_size(mode))
            .saturating_add(self.c.serialized_size(mode))
            .saturating_add(self.a_arith.serialized_size(mode))
            .saturating_add(self.b_arith.serialized_size(mode))
            .saturating_add(self.c_arith.serialized_size(mode))
    }
}

impl<F: PrimeField, MM: SNARKMode> snarkvm_utilities::Valid for Circuit<F, MM> {
    fn check(&self) -> Result<(), SerializationError> {
        Ok(())
    }

    fn batch_check<'a>(_batch: impl Iterator<Item = &'a Self> + Send) -> Result<(), SerializationError> {
        Ok(())
    }
}

impl<F: PrimeField, MM: SNARKMode> CanonicalDeserialize for Circuit<F, MM> {
    fn deserialize_with_mode<R: Read>(
        mut reader: R,
        compress: Compress,
        validate: Validate,
    ) -> Result<Self, SerializationError> {
        let index_info: CircuitInfo = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let a = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let b = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let c = CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?;
        let id = Self::hash(&index_info, &a, &b, &c)?;
        Ok(Circuit {
            index_info,
            a,
            b,
            c,
            a_arith: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            b_arith: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            c_arith: CanonicalDeserialize::deserialize_with_mode(&mut reader, compress, validate)?,
            _mode: PhantomData,
            id,
        })
    }
}
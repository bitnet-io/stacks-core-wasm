// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2022 Stacks Open Internet Foundation
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use hashbrown::HashMap;
use stacks_common::types::StacksEpochId;

use crate::vm::analysis::errors::{CheckError, CheckErrors, CheckResult};
use crate::vm::types::signatures::CallableSubtype;
use crate::vm::types::{
    ListTypeData, SequenceSubtype, TraitIdentifier, TupleTypeSignature, TypeSignature,
};
use crate::vm::{ClarityName, ClarityVersion, SymbolicExpression, MAX_CONTEXT_DEPTH};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TypeMap {
    map: HashMap<u64, TypeSignature>,
}

pub struct TypingContext<'a> {
    pub epoch: StacksEpochId,
    pub clarity_version: ClarityVersion,
    pub variable_types: HashMap<ClarityName, TypeSignature>,
    pub traits_references: HashMap<ClarityName, TraitIdentifier>,
    pub parent: Option<&'a TypingContext<'a>>,
    pub depth: u16,
}

impl Default for TypeMap {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeMap {
    pub fn new() -> TypeMap {
        TypeMap {
            map: HashMap::new(),
        }
    }

    pub fn set_type(
        &mut self,
        expr: &SymbolicExpression,
        type_sig: TypeSignature,
    ) -> CheckResult<()> {
        if self.map.insert(expr.id, type_sig).is_some() {
            Err(CheckError::new(CheckErrors::TypeAlreadyAnnotatedFailure))
        } else {
            Ok(())
        }
    }

    pub fn get_type(&self, expr: &SymbolicExpression) -> Option<&TypeSignature> {
        self.map.get(&expr.id)
    }

    /// Concretize tries to [concretize] all the types in the TypeMap.
    ///
    /// This is needed for Clarity-Wasm where all types should have a defined representation
    /// in memory. Since [ListUnionType] doesn't have one, we need to concretize it.
    ///
    /// [concretize]: TypeSignature::concretize
    /// [ListUnionType]: TypeSignature::ListUnionType
    pub fn concretize(&mut self) -> CheckResult<()> {
        for ty in self.map.values_mut() {
            *ty = ty.clone().concretize_deep()?;
        }
        Ok(())
    }
}

impl<'a> TypingContext<'a> {
    pub fn new(epoch: StacksEpochId, clarity_version: ClarityVersion) -> TypingContext<'static> {
        TypingContext {
            epoch,
            clarity_version,
            variable_types: HashMap::new(),
            traits_references: HashMap::new(),
            depth: 0,
            parent: None,
        }
    }

    pub fn extend(&self) -> CheckResult<TypingContext> {
        if self.depth >= MAX_CONTEXT_DEPTH {
            Err(CheckError::new(CheckErrors::MaxContextDepthReached))
        } else {
            Ok(TypingContext {
                epoch: self.epoch,
                clarity_version: self.clarity_version,
                variable_types: HashMap::new(),
                traits_references: HashMap::new(),
                parent: Some(self),
                depth: self.depth + 1,
            })
        }
    }

    pub fn lookup_variable_type(&self, name: &str) -> Option<&TypeSignature> {
        match self.variable_types.get(name) {
            Some(value) => Some(value),
            None => match self.parent {
                Some(parent) => parent.lookup_variable_type(name),
                None => None,
            },
        }
    }

    pub fn add_variable_type(
        &mut self,
        name: ClarityName,
        var_type: TypeSignature,
        clarity_version: ClarityVersion,
    ) {
        // Beginning in Clarity 2, traits can be bound.
        if clarity_version >= ClarityVersion::Clarity2 {
            if let TypeSignature::CallableType(CallableSubtype::Trait(trait_id)) = var_type {
                self.traits_references.insert(name, trait_id);
            } else {
                self.variable_types.insert(name, var_type);
            }
        } else {
            self.variable_types.insert(name, var_type);
        }
    }

    pub fn add_trait_reference(&mut self, name: &ClarityName, value: &TraitIdentifier) {
        self.traits_references.insert(name.clone(), value.clone());
    }

    pub fn lookup_trait_reference_type(&self, name: &str) -> Option<&TraitIdentifier> {
        match self.traits_references.get(name) {
            Some(value) => Some(value),
            None => match self.parent {
                Some(parent) => parent.lookup_trait_reference_type(name),
                None => None,
            },
        }
    }
}

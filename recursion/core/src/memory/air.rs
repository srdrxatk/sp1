use core::mem::size_of;
use p3_air::{Air, BaseAir};
use p3_field::PrimeField32;
use p3_matrix::dense::RowMajorMatrix;
use p3_matrix::Matrix;
use sp1_core::air::{AirInteraction, SP1AirBuilder};
use sp1_core::lookup::InteractionKind;
use sp1_core::{air::MachineAir, utils::pad_to_power_of_two};
use std::borrow::{Borrow, BorrowMut};

use super::columns::MemoryInitCols;
use crate::memory::MemoryChipKind;
use crate::memory::MemoryGlobalChip;
use crate::runtime::{ExecutionRecord, RecursionProgram};

pub(crate) const NUM_MEMORY_INIT_COLS: usize = size_of::<MemoryInitCols<u8>>();

#[allow(dead_code)]
impl MemoryGlobalChip {
    pub fn new(kind: MemoryChipKind) -> Self {
        Self { kind }
    }
}

impl<F: PrimeField32> MachineAir<F> for MemoryGlobalChip {
    type Record = ExecutionRecord<F>;
    type Program = RecursionProgram<F>;

    fn name(&self) -> String {
        match self.kind {
            MemoryChipKind::Init => "MemoryInit".to_string(),
            MemoryChipKind::Finalize => "MemoryFinalize".to_string(),
        }
    }

    fn generate_dependencies(&self, _: &Self::Record, _: &mut Self::Record) {
        // This is a no-op.
    }

    #[allow(unused_variables)]
    fn generate_trace(
        &self,
        input: &ExecutionRecord<F>,
        _output: &mut ExecutionRecord<F>,
    ) -> RowMajorMatrix<F> {
        let rows = match self.kind {
            MemoryChipKind::Init => {
                let addresses = &input.first_memory_record;
                addresses
                    .iter()
                    .map(|(addr, value)| {
                        let mut row = [F::zero(); NUM_MEMORY_INIT_COLS];
                        let cols: &mut MemoryInitCols<F> = row.as_mut_slice().borrow_mut();
                        cols.addr = *addr;
                        cols.timestamp = F::zero();
                        cols.value = *value;
                        cols.is_real = F::one();
                        row
                    })
                    .collect::<Vec<_>>()
            }
            MemoryChipKind::Finalize => input
                .last_memory_record
                .iter()
                .map(|(addr, timestamp, value)| {
                    let mut row = [F::zero(); NUM_MEMORY_INIT_COLS];
                    let cols: &mut MemoryInitCols<F> = row.as_mut_slice().borrow_mut();
                    cols.addr = *addr;
                    cols.timestamp = *timestamp;
                    cols.value = *value;
                    cols.is_real = F::one();
                    row
                })
                .collect::<Vec<_>>(),
        };

        let mut trace = RowMajorMatrix::new(
            rows.into_iter().flatten().collect::<Vec<_>>(),
            NUM_MEMORY_INIT_COLS,
        );

        pad_to_power_of_two::<NUM_MEMORY_INIT_COLS, F>(&mut trace.values);

        trace
    }

    fn included(&self, shard: &Self::Record) -> bool {
        match self.kind {
            MemoryChipKind::Init => !shard.first_memory_record.is_empty(),
            MemoryChipKind::Finalize => !shard.last_memory_record.is_empty(),
        }
    }
}

impl<F> BaseAir<F> for MemoryGlobalChip {
    fn width(&self) -> usize {
        NUM_MEMORY_INIT_COLS
    }
}

impl<AB> Air<AB> for MemoryGlobalChip
where
    AB: SP1AirBuilder,
{
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let local = main.row_slice(0);
        let local: &MemoryInitCols<AB::Var> = (*local).borrow();

        match self.kind {
            MemoryChipKind::Init => {
                builder.send(AirInteraction::new(
                    vec![
                        local.timestamp.into(),
                        local.addr.into(),
                        local.value[0].into(),
                        local.value[1].into(),
                        local.value[2].into(),
                        local.value[3].into(),
                    ],
                    local.is_real.into(),
                    InteractionKind::Memory,
                ));
            }
            MemoryChipKind::Finalize => {
                builder.receive(AirInteraction::new(
                    vec![
                        local.timestamp.into(),
                        local.addr.into(),
                        local.value[0].into(),
                        local.value[1].into(),
                        local.value[2].into(),
                        local.value[3].into(),
                    ],
                    local.is_real.into(),
                    InteractionKind::Memory,
                ));
            }
        };
    }
}

use std::marker::PhantomData;

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Chip, Layouter, SimpleFloorPlanner, Value},
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Selector},
    poly::Rotation, pasta::Fp, dev::MockProver,
};

trait FiboInstructions<F: FieldExt>: Chip<F> {}

#[derive(Debug, Clone)]
struct FiboConfig {
    pub advice: [Column<Advice>; 3],
    pub selector: Selector,
}

#[derive(Debug, Clone)]
struct ACell<F: FieldExt>(AssignedCell<F, F>);

struct FiboChip<F: FieldExt> {
    config: FiboConfig,
    _marker: PhantomData<F>,
}

impl<F: FieldExt> FiboChip<F> {
    fn assign_first_row(
        &self,
        mut layouter: impl Layouter<F>,
        a: Value<F>,
        b: Value<F>,
    ) -> Result<(ACell<F>, ACell<F>, ACell<F>), Error> {
        layouter.assign_region(
            || "first row",
            |mut region| {
                self.config.selector.enable(&mut region, 0)?;

                let a_cell = region
                    .assign_advice(|| "a", self.config.advice[0], 0, || a)
                    .map(ACell)?;

                let b_cell = region
                    .assign_advice(|| "b", self.config.advice[1], 0, || b)
                    .map(ACell)?;

                let c_val = a.and_then(|a| b.map(|b| a + b));

                let c_cell = region
                    .assign_advice(|| "c", self.config.advice[2], 0, || c_val)
                    .map(ACell)?;

                Ok((a_cell, b_cell, c_cell))
            },
        )
    }

    fn assign_row(&self, mut layouter: impl Layouter<F>, prev_b: &ACell<F>, prev_c: &ACell<F>) -> Result<ACell<F>, Error> {
        layouter.assign_region(
            || "next row", 
            |mut region| -> Result<ACell<F>, Error> {
                self.config.selector.enable(&mut region, 0)?;

                prev_b.0.copy_advice(|| "a", &mut region, self.config.advice[0], 0)?;
                prev_c.0.copy_advice(|| "b", &mut region, self.config.advice[1], 0)?;

                let c_val = prev_b.0.value().and_then(|b| {
                    prev_c.0.value().map(|c| *b + *c)
                });

                let c_cell = region.assign_advice( || "c", self.config.advice[2], 0, || c_val).map(ACell)?;

                Ok(c_cell)
            }
        )
    }

    fn configure(meta: &mut ConstraintSystem<F>, advices: [Column<Advice>; 3]) -> FiboConfig {
        let [col_a, col_b, col_c] = advices;
        let selector = meta.selector();

        meta.enable_equality(col_a);
        meta.enable_equality(col_b);
        meta.enable_equality(col_c);

        meta.create_gate("add", |meta| {
            let s = meta.query_selector(selector);
            let a = meta.query_advice(col_a, Rotation::cur());
            let b = meta.query_advice(col_b, Rotation::cur());
            let c = meta.query_advice(col_c, Rotation::cur());
            vec![(s * (a + b - c))]
        });

        FiboConfig {
            advice: [col_a, col_b, col_c],
            selector,
        }
    }

    fn construct(config: FiboConfig) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }
}

#[derive(Default)]
struct FiboCircuit<F> {
    pub a: Value<F>,
    pub b: Value<F>,
}

impl<F: FieldExt> Circuit<F> for FiboCircuit<F> {
    type Config = FiboConfig;
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advices = [
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
        ];
        FiboChip::configure(meta, advices)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<F>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let chip = FiboChip::<F>::construct(config);

        let (_, mut prev_b,mut prev_c) = chip.assign_first_row(layouter.namespace(|| "first row"), self.a, self.b).unwrap();

        for _i in 3..10 {
            let c_cell = chip.assign_row(
                layouter.namespace(|| "next row"),
                &prev_b, 
                &prev_c
            )?;

            prev_b = prev_c;
            prev_c = c_cell;
        }
        Ok(())
    }
}
fn main() {
    let k = 4; 
    let a= Fp::from(1);
    let b =Fp::from(1);

    let circuit = FiboCircuit {
        a: Value::known(a),
        b: Value::known(b)
    };

    let prover = MockProver::run(k, &circuit, vec![]).unwrap();
    prover.assert_satisfied();
}

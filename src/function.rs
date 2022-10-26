// x ^ 3 + x + 5 = 35
// | x | constaint | selector_power | add_selector| mul_selector |
// gate only x: (x - constant)* selector
// gate power: (x * x -c) * selector_power
// gate plus: (a + b - c) * add_selector

use std::marker::PhantomData;

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Chip, Layouter, SimpleFloorPlanner, Value},
    pasta::Fp,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Selector},
    poly::Rotation, dev::MockProver,
};

trait SimpleFunctionInstructions<F: FieldExt>: Chip<F> {
    type Num;

    fn load_add(
        &self,
        layouter: impl Layouter<F>,
        x: Value<F>,
        y: Value<F>,
    ) -> Result<(Self::Num, Self::Num, Self::Num), Error>;
    fn load_mul(
        &self,
        layouter: impl Layouter<F>,
        x: Value<F>,
        y: Value<F>,
    ) -> Result<(Self::Num, Self::Num, Self::Num), Error>;

    fn load_assign(
        &self,
        layouter: impl Layouter<F>,
        x: Value<F>,
        y: Value<F>,
    ) -> Result<Self::Num, Error>;
}

#[derive(Clone, Debug)]
struct SimpleFunctionConfig {
    x: Column<Advice>,
    y: Column<Advice>,
    z: Column<Advice>,
    s_add: Selector,
    s_mul: Selector,
}

struct SimpleFunctionChip<F: FieldExt> {
    config: SimpleFunctionConfig,
    _market: PhantomData<F>,
}

impl<F: FieldExt> Chip<F> for SimpleFunctionChip<F> {
    type Config = SimpleFunctionConfig;
    type Loaded = ();
    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}

impl<F: FieldExt> SimpleFunctionChip<F> {
    fn construct(config: <Self as Chip<F>>::Config) -> Self {
        Self {
            config,
            _market: PhantomData,
        }
    }

    fn configure(
        meta: &mut ConstraintSystem<F>,
        x: Column<Advice>,
        y: Column<Advice>,
        z: Column<Advice>,
    ) -> <Self as Chip<F>>::Config {
        meta.enable_equality(x);
        meta.enable_equality(y);
        meta.enable_equality(z);

        let s_add = meta.selector();

        meta.create_gate("add", |meta| {
            let left = meta.query_advice(x, Rotation::cur());
            let right = meta.query_advice(y, Rotation::cur());
            let out = meta.query_advice(z, Rotation::cur());

            let s = meta.query_selector(s_add);

            vec![s * (left + right - out)]
        });

        let s_mul = meta.selector();
        meta.create_gate("mul", |meta| {
            let left = meta.query_advice(x, Rotation::cur());
            let right = meta.query_advice(y, Rotation::cur());
            let out = meta.query_advice(z, Rotation::cur());

            let s = meta.query_selector(s_mul);

            vec![s * (left * right - out)]
        });
        SimpleFunctionConfig {
            x,
            y,
            z,
            s_add,
            s_mul,
        }
    }
}

#[derive(Clone)]
struct Number<F: FieldExt>(AssignedCell<F, F>);

impl<F: FieldExt> SimpleFunctionInstructions<F> for SimpleFunctionChip<F> {
    type Num = Number<F>;

    fn load_add(
        &self,
        mut layouter: impl Layouter<F>,
        x: Value<F>,
        y: Value<F>,
    ) -> Result<(Self::Num, Self::Num, Self::Num), Error> {
        let config = self.config();

        layouter.assign_region(
            || "add",
            |mut region| {
                self.config().s_add.enable(&mut region, 0)?;
                let x_cell = region.assign_advice(|| "a", config.x, 0, || x).map(Number)?;
                let y_cell = region.assign_advice(|| "b", config.y, 0, || y).map(Number)?;
                let z = x.and_then(|x_val| y.map(|y_val| x_val + y_val));
                let z_cell = region.assign_advice(|| "c", config.z, 0, || z).map(Number)?;
                Ok((x_cell, y_cell, z_cell))
            },
        )
    }

    fn load_mul(
        &self,
        mut layouter: impl Layouter<F>,
        x: Value<F>,
        y: Value<F>,
    ) -> Result<(Self::Num, Self::Num, Self::Num), Error> {
        let config = self.config();

        layouter.assign_region(
            || "mul",
            |mut region| {
                self.config().s_mul.enable(&mut region, 0)?;
                let x_cell = region.assign_advice(|| "", config.x, 0, || x).map(Number)?;
                let y_cell = region.assign_advice(|| "", config.y, 0, || y).map(Number)?;
                let z = x.and_then(|x_val| y.map(|y_val| x_val * y_val));

                let z_cell = region.assign_advice(|| "", config.z, 0, || z).map(Number)?;
                Ok((x_cell, y_cell, z_cell))
            },
        )
    }

    fn load_assign(&self, mut layouter: impl Layouter<F>, x: Value<F>, y: Value<F>) -> Result<Self::Num, Error>{
        let config = self.config();
        layouter.assign_region(
            || "equal",
            |mut region| {
                self.config().s_add.enable(&mut region, 0)?;
                let x_cell = region.assign_advice(|| "", config.x, 0, || x).map(Number)?;
                region.assign_advice(|| "", config.y, 0, || Value::known(FieldExt::from_u128(0))).map(Number)?;
                region.assign_advice(|| "", config.z, 0, || y).map(Number)?;
                Ok(x_cell)
            },
        )
    }
}

#[derive(Default)]
struct FunctionCircuit<F: FieldExt> {
    x: Value<F>,
}

impl<F: FieldExt> Circuit<F> for FunctionCircuit<F> {
    type Config = SimpleFunctionConfig;
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let x = meta.advice_column();
        let y = meta.advice_column();
        let z = meta.advice_column();
        SimpleFunctionChip::configure(meta, x, y, z)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let chip = SimpleFunctionChip::<F>::construct(config);

        // | x^3 + x + 5 = 35 |

        // mul gate
        let (_, _, x) = chip.load_mul(
            layouter.namespace(|| "mul"),
            self.x,
            Value::known(FieldExt::from_u128(1)),
        )?;
        let (_, _, x_square) = chip.load_mul(
            layouter.namespace(|| "mul"),
            x.0.value().map(|x| *x),
            self.x,
        )?;
        let (_, _, x_cube) = chip.load_mul(
            layouter.namespace(|| "mul"),
            x_square.0.value().map(|x_val| *x_val),
            self.x,
        )?;

        // add gate
        let (_, _, tmp1) = chip.load_add(
            layouter.namespace(|| "add"),
            x_cube.0.value().map(|x_val| *x_val),
            self.x,
        )?;
        let (_, _, tmp2) = chip.load_add(
            layouter.namespace(|| "add"),
            tmp1.0.value().map(|x_val| *x_val),
            Value::known(FieldExt::from_u128(5)),
        )?;

        chip.load_assign(layouter, tmp2.0.value().map(|x| *x), Value::known(FieldExt::from_u128(35)))?;
        Ok(())
    }
}
fn main() {
    let k = 4;
    let x = Fp::from(3);

    let circuit = FunctionCircuit {
        x: Value::known(x),
    };

    let prover = MockProver::run(k, &circuit, vec![]).unwrap();
    prover.assert_satisfied();

    use plotters::prelude::*;
    let root = BitMapBackend::new("./target/function.png", (1024, 768)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    let root = root
        .titled("Function", ("sans-serif", 60))
        .unwrap();

    halo2_proofs::dev::CircuitLayout::default()
        // .show_labels(false)
        // Render the circuit onto your area!
        // The first argument is the size parameter for the circuit.
        .render(4, &circuit, &root)
        .unwrap();
}

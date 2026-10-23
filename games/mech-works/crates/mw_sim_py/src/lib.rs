//! `mw_sim_py`: the Mech Works training environment for Python, as a batch
//! of [`Env`]s stepped together (in parallel, with rayon) in one call.
//!
//! ```python
//! import mw_sim_py
//! env = mw_sim_py.VecEnv(n=1024, seed=0)
//! obs = env.reset()                      # (n, OBS) float32
//! obs, reward, terminated, truncated, terms = env.step(actions)  # actions (n, 12)
//! ```
//!
//! An env whose episode ended is reset at once: `obs` is then its new
//! episode's first observation, and `final_obs()` holds the last one.

#![cfg(not(target_arch = "wasm32"))]

use mw_sim::env::{Curriculum, Env, OBS, REWARD_TERMS};
use mw_sim::mech::{BODY_PARAMS, JOINTS};
use numpy::ndarray::{Array1, Array2};
use numpy::{IntoPyArray, PyArray1, PyArray2, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rayon::prelude::*;

type Step<'py> = (
    Bound<'py, PyArray2<f32>>,
    Bound<'py, PyArray1<f32>>,
    Bound<'py, PyArray1<bool>>,
    Bound<'py, PyArray1<bool>>,
    Bound<'py, PyArray2<f32>>,
);

#[pyclass]
struct VecEnv {
    envs: Vec<Env>,
    final_obs: Array2<f32>,
}

#[pymethods]
impl VecEnv {
    #[new]
    #[pyo3(signature = (n, seed=0))]
    fn new(n: usize, seed: u64) -> Self {
        let envs = (0..n)
            .into_par_iter()
            .map(|i| Env::new(seed.wrapping_mul(1_000_003).wrapping_add(i as u64)))
            .collect();
        Self {
            envs,
            final_obs: Array2::zeros((n, OBS)),
        }
    }

    #[getter]
    fn num_envs(&self) -> usize {
        self.envs.len()
    }

    #[classattr]
    fn obs_dim() -> usize {
        OBS
    }

    #[classattr]
    fn act_dim() -> usize {
        JOINTS
    }

    #[classattr]
    fn body_params_dim() -> usize {
        BODY_PARAMS
    }

    #[classattr]
    fn reward_terms() -> Vec<&'static str> {
        REWARD_TERMS.to_vec()
    }

    /// Resets every env; returns the observations.
    fn reset<'py>(&mut self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
        py.allow_threads(|| self.envs.par_iter_mut().for_each(|e| e.reset()));
        self.observe(py)
    }

    fn observe<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
        let mut out = Array2::zeros((self.envs.len(), OBS));
        for (i, e) in self.envs.iter().enumerate() {
            out.row_mut(i).assign(&Array1::from_iter(e.observe()));
        }
        out.into_pyarray(py)
    }

    /// Steps every env with its row of `actions`.
    fn step<'py>(
        &mut self,
        py: Python<'py>,
        actions: PyReadonlyArray2<'py, f32>,
    ) -> PyResult<Step<'py>> {
        let a = actions.as_array();
        let n = self.envs.len();
        if a.shape() != [n, JOINTS] {
            return Err(PyValueError::new_err(format!(
                "actions must be ({n}, {JOINTS}), got {:?}",
                a.shape()
            )));
        }
        let rows: Vec<[f32; JOINTS]> = (0..n).map(|i| std::array::from_fn(|j| a[[i, j]])).collect();
        let results: Vec<_> = py.allow_threads(|| {
            self.envs
                .par_iter_mut()
                .zip(rows.par_iter())
                .map(|(e, act)| {
                    let r = e.step(act);
                    let last = if r.terminated || r.truncated {
                        let last = e.observe();
                        e.reset();
                        Some(last)
                    } else {
                        None
                    };
                    (r, last, e.observe())
                })
                .collect()
        });
        let mut obs = Array2::zeros((n, OBS));
        let mut reward = Array1::zeros(n);
        let mut terminated = Array1::from_elem(n, false);
        let mut truncated = Array1::from_elem(n, false);
        let mut terms = Array2::zeros((n, REWARD_TERMS.len()));
        for (i, (r, last, o)) in results.into_iter().enumerate() {
            obs.row_mut(i).assign(&Array1::from_iter(o));
            reward[i] = r.reward;
            terminated[i] = r.terminated;
            truncated[i] = r.truncated;
            terms.row_mut(i).assign(&Array1::from_iter(r.terms));
            if let Some(last) = last {
                self.final_obs.row_mut(i).assign(&Array1::from_iter(last));
            }
        }
        Ok((
            obs.into_pyarray(py),
            reward.into_pyarray(py),
            terminated.into_pyarray(py),
            truncated.into_pyarray(py),
            terms.into_pyarray(py),
        ))
    }

    /// The last observation of each env whose episode just ended (rows of
    /// the others are stale).
    fn final_obs<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
        self.final_obs.clone().into_pyarray(py)
    }

    /// Sets how wide the randomization is, each 0 to 1; takes effect at
    /// each env's next reset.
    #[pyo3(signature = (body=0.0, damage=0.0, pushes=0.0, speed=0.3))]
    fn set_curriculum(&mut self, body: f32, damage: f32, pushes: f32, speed: f32) {
        let c = Curriculum {
            body,
            damage,
            pushes,
            speed,
        };
        for e in &mut self.envs {
            e.curriculum = c;
        }
    }

    /// The true body parameters of each env, (n, BODY_PARAMS): what the
    /// estimator learns to infer.
    fn body_params<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
        let mut out = Array2::zeros((self.envs.len(), BODY_PARAMS));
        for (i, e) in self.envs.iter().enumerate() {
            out.row_mut(i)
                .assign(&Array1::from_iter(e.sim.params.to_vec()));
        }
        out.into_pyarray(py)
    }

    /// Each env's step count in its episode.
    fn episode_steps<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<u32>> {
        Array1::from_iter(self.envs.iter().map(|e| e.steps)).into_pyarray(py)
    }
}

#[pymodule]
fn mw_sim_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VecEnv>()?;
    m.add("OBS", OBS)?;
    m.add("PROPRIO", mw_sim::env::PROPRIO)?;
    m.add("JOINTS", JOINTS)?;
    m.add("BODY_PARAMS", BODY_PARAMS)?;
    m.add("POLICY_HZ", mw_sim::env::POLICY_HZ)?;
    Ok(())
}

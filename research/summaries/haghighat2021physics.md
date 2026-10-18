# Physics-informed neural network simulation of multiphase poroelasticity using stress-split sequential training
Ehsan Haghighat, Danial Amini, Ruben Juanes · arXiv 2021 · https://doi.org/10.1016/j.cma.2022.115141 · relevance: low · basis: abstract

**Problem:** Physics-informed neural networks (PINNs) have received significant attention as a unified framework for forward, inverse, and surrogate modeling of problems governed by partial differential equations (PDEs).
**Approach:** In this work, we present a PINN approach to solving the equations of coupled flow and deformation in porous media for both single-phase and multiphase flow. Additionally, we propose a sequential training approach based on the stress-split algorithms of poromechanics.
**Tools and scale:** Mentions: simulation (details not in abstract).
**Results:** Due to the dynamics of the problem, we find that incorporating multiple differential relations into the loss function results in an unstable optimization problem, meaning that sometimes it converges to the trivial null solution, other times it moves very far from the expected solution. Notably, we find that sequential training based on stress-split performs well for different problems, while the classical strain-split algorithm shows an unstable behaviour similar to what is reported in the context of finite element solvers.
**Limitations and future work (as stated by the authors):**
- "Training PINNs for forward problems, however, pose significant challenges, mainly because of the complex non-convex and multi-objective loss function."
**Overlap with our engine:** none — adjacent background; does not implement simulation, SCMS backend, or closed-loop evaluation comparable to the engine (judged from abstract).
**Novelty implications:** Rules out no candidate-direction claim on the available evidence; background reference only.

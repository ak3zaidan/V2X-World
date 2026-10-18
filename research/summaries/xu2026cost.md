# Cost Modeling and Configuration Optimization for Large-Scale VANET Co-Simulation
Yang Xu, Zhen Cai, Haozheng Han, Xuqiang Shao · Applied Sciences 2026 · https://doi.org/10.3390/app16073264 · relevance: high · basis: full text

**Problem:** Vehicular Ad Hoc Network (VANET) traffic–network co-simulation is a foundational methodology for the engineering evaluation of vehicle-to-everything (V2X) protocols and cooperative Intelligent Transportation System (ITS) applications before field deployment. However, with research objectives and experimental conditions varying widely, existing studies still lack a systematic paradigm for parameter configuration and experimental workflows.
**Approach:** To address these issues, this paper proposes a simulation cost modeling and configuration optimization methodology for traffic–network co-simulation. By profiling and structurally modeling key overheads, such as initialization and traffic- and network-side execution, we characterize how traffic, network, and control parameters jointly affect total simulation overhead.
**Tools and scale:** SUMO + Veins + OMNeT++ representative urban co-simulation; cost profiling of initialization, traffic-side and network-side execution.
**Results:** Simulation results show that the proposed method reduces simulation overhead while keeping conclusions on key performance metrics consistent, thereby providing a more efficient and statistically credible evaluation basis for application-oriented urban VANET studies related to traffic safety, transportation efficiency, and wireless-system performance.
**Limitations and future work (as stated by the authors):**
- Validation only on one representative urban scenario; future work on "more diverse road–network topologies, traffic-demand patterns, and communication settings".
- Extend to "adaptive configuration across different protocol stacks and application-oriented workloads".
- Data and scripts not publicly archived.
**Overlap with our engine:** partial — Systematic configuration of co-simulation parameters under statistical-validity constraints; relates to our ~300 settings and the LLM configuration agent, and to counterfactual-rerun efficiency.
**Novelty implications:** Cannot claim first systematic cost/statistical-power model for VANET co-simulation configuration; single-process deterministic engines and paired (same-seed) designs that reduce required replications remain open.

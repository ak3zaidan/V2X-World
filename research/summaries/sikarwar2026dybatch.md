# DyBatch: Message Prioritization and Priority-Driven Dynamic Batch Verification in Large-Scale IoV Networks
Himani Sikarwar, Ankur Nahar, Debasis Das · IEEE Transactions on Vehicular Technology 2026 · https://doi.org/10.1109/tvt.2025.3621547 · relevance: medium · basis: abstract

**Problem:** In enhancing transportation, the Internet of Vehicles (IoV) facilitates real-time vehicle-infrastructure interactions, improving traffic management and safety, crucial for autonomous vehicle development. However, high-traffic IoV networks face challenges with traditional Batch Message Verification (BMV) methods, often leading to critical alerts being unverified and dropped due to a lack of prioritization.
**Approach:** Addressing this, we introduce an enhanced verification model, “DyBatch: Message Prioritization and Dynamic Batch Verification in Large-scale IoV Networks”, designed for Edge-computing enabled IoV (e-IoV) networks.
**Tools and scale:** Not stated in the abstract; appears to be analytical/cryptographic or review work.
**Results:** In enhancing transportation, the Internet of Vehicles (IoV) facilitates real-time vehicle-infrastructure interactions, improving traffic management and safety, crucial for autonomous vehicle development. Utilizing the Multi-precision Integer and Rational Arithmetic Cryptographic Library (MIRACL) and Java 8 on Jetson Nano and Raspberry Pi 4, our analysis reveals a 90% reduction in verification delays, a 91% ...
**Limitations and future work (as stated by the authors):**
- However, high-traffic IoV networks face challenges with traditional Batch Message Verification (BMV) methods, often leading to critical alerts being unverified and dropped due to a lack of prioritization.
- DyBatch effectively tackles message loss by classifying Basic Alert Messages (BAMs) based on urgency through a flag-based mechanism, coupled with a flexible batch size concept to overcome fixed-size limitations.
**Overlap with our engine:** partial — priority-driven dynamic batch verification under load; relates to verification-policy design (direction 2).
**Novelty implications:** Does not remove any of our claims; it is a related point of comparison. We cannot claim to be first on the specific mechanism it covers; the joint closed-loop, multi-layer evaluation remains open as far as this abstract shows.

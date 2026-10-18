# Effects of and Defenses Against Adversarial Attacks on a Traffic Light Classification CNN
Morris Wan, Meng Han, Lin Li, Zhigang Li et al. · Proceedings of the 2020 ACM Southeast Conference 2020 · https://doi.org/10.1145/3374135.3385288 · relevance: high · basis: abstract

**Problem:** Convolutional Neural networks (CNNs) have become more and more prevalent in applications such as image recognition and autonomous driving.
**Approach:** However, recent literature shows that a lot of these deep-learning algorithms are susceptible to small perturbations to their input known as adversarial attacks. In this project, adversarial attack algorithms and defenses were implemented and evaluated using real traffic lights data and model.
**Tools and scale:** Not stated in the available text unless given above (topic: traffic signal attacks).
**Results:** It was found that the particular model architecture was only really susceptible to the spatial attack (56% decrease in accuracy on perturbed images) and the C&W attack (0% accuracy on perturbed images). This significantly helped against the spatial attack (22.5% decrease in success rate).
**Limitations and future work (as stated by the authors):**
- None stated in the available text.
**Overlap with our engine:** partial — attacks on signal control; our engine simulates signals and in-sim attacks
**Novelty implications:** Does not affect novelty claims of the candidate directions beyond background in traffic signal attacks.

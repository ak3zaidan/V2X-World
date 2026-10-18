# Machine Learning Aided Path Loss Estimator and Jammer Detector for Heterogeneous Vehicular Networks
Bugra Turan, Ali Uyrus, O. Nuri Koc, Emrah Kar et al. · 2021 IEEE Global Communications Conference (GLOBECOM) 2021 · https://doi.org/10.1109/globecom46510.2021.9685428 · relevance: medium · basis: abstract

**Problem:** Heterogeneous vehicular communications aim to improve the reliability, security and delay performance of vehicle-to-vehicle (V2V) communications, by utilizing multiple commu-nication technologies. Predicting the path loss through conventional fitting based models and radio frequency (RF) jamming detection through rule based models of different communication schemes fail to address comprehensive mobility and jamming scenarios.
**Approach:** In this paper, we propose a machine learning based adaptive link quality estimation and jamming detection scheme for the optimum selection and aggregation of IEEE 802.11p and Vehicular Visible Light Communications (V-VLC) technologies targeting reliable V2V communications. We propose to use Random Forest regression and classifier based algorithms, where multiple individual learners with diversity are trained by using measurement data and the final result is obtained by averaging outputs of all learners.
**Tools and scale:** Mentioned in abstract: IEEE 802.11p, real-world. (From the abstract only.)
**Results:** We test our framework on real-world road measurement data, demonstrating up to 2.34 dB and 0.56 dB Mean Absolute Error (MAE) improvement for V-VLC and IEEE 802.11p path loss prediction compared to fitting based models, respectively. The proposed jamming presence detection scheme yields 88.3% accuracy to detect noise interference injection for IEEE 802.11p links, yielding 3% better prediction performance than previously proposed deep convolutional neural network (DCNN) based scheme.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** partial — ML path-loss estimator and jammer detector for heterogeneous V2V.
**Novelty implications:** Jamming detection at PHY; not PKI.

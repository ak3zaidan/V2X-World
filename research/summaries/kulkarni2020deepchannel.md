# DeepChannel: Wireless Channel Quality Prediction Using Deep Learning
Adita Kulkarni, Anand Seetharam, Arti Ramesh, J. Dinal Herath · IEEE Transactions on Vehicular Technology 2020 · https://doi.org/10.1109/tvt.2019.2949954 · relevance: medium · basis: abstract

**Problem:** Accurately modeling and predicting wireless channelquality variations is essential for a number of networking applications such as scheduling and improved video streaming over 4G LTE networks and bit rate adaptation for improved performance in WiFi networks. In this paper, we design DeepChannel, an encoder-decoder based sequence-to-sequence deep learning model that is capable of predicting future wireless signal strength variations based on past signal strength data.
**Approach:** We consider two different versions of DeepChannel; the first and second versions use LSTM and GRU as their basic cell structure, respectively. We compare the performance (i.e., the root mean squared error, mean absolute error and relative error of future predictions) of DeepChannel with respect to two baselines—i) linear regression, and ii) ARIMA for multiple networks and communication standards.
**Tools and scale:** Not stated in the abstract. (From the abstract only.)
**Results:** In particular, we consider 4G LTE, WiFi, WiMAX, an industrial network operating in the 5.8 GHz range, and Zigbee networks operating under varying levels of user mobility and observe that DeepChannel provides significantly superior performance.
**Limitations and future work (as stated by the authors):**
- Not stated in the abstract.
**Overlap with our engine:** none — Adjacent background (AI for networks (adjacent)); no direct overlap with the engine's traffic+radio+SCMS joint simulation.
**Novelty implications:** Does not restrict any candidate claim.

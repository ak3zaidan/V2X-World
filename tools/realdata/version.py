"""Version of the real-data pipeline. Bump it whenever a stage changes a number."""
VERSION = "1.2.0"
USER_AGENT = "v2xw-realdata/" + VERSION  # no personal identity in any request
# The version that froze tools/realdata/frozen/classifier-v1.json. The class stages (normalise,
# pair, classify, the headline cell) have not changed since, so a refit still reproduces it byte
# for byte; 1.1.0 and 1.2.0 changed only the timing-model fit (schemas v2xw.bsm_timing.v2 and v3).
CLASSIFIER_VERSION = "1.0.0"

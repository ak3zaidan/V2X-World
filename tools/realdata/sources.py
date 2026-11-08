"""The pinned queries (stage 1). Changing any of them changes the manifest, so bump the
tool version. Row ids (`:id`) end every $order so offset paging is a total order."""

TAMPA, OREM, WYOMING, NYC = "nm7w-nvbm", "ev64-jn8c", "7qu7-j8rx", "vxmm-9bi5"

DATASETS = {
    TAMPA: {"label": "Tampa CV Pilot BSM Sample", "licence": "CC BY-SA 4.0", "role": "fit"},
    OREM: {"label": "UDOT/Panasonic BSM, Orem UT", "licence": "none stated", "role": "fit"},
    WYOMING: {"label": "Wyoming CV Pilot BSM One Day Sample", "licence": "CC BY-SA 4.0", "role": "held_out"},
    NYC: {"label": "NYC CV Pilot EVENT One Month Sample", "licence": "NYC Terms of Use + CC BY 4.0",
          "role": "separate"},
}

OREM_DAYS = ["2025-02-0%d" % d for d in range(2, 9)]
NYC_EVENTS = 600  # stated subset: the first 600 events by row id (~0.3 MB each)

_TAMPA_COLS = ["metadata_recordgeneratedat", "coredata_id", "coredata_secmark", "coredata_msgcnt",
               "coredata_lat", "coredata_long", "coredata_speed", "coredata_heading",
               "coredata_accelset_long", "coredata_accuracy_semimajor", "coredata_accuracy_semiminor",
               "coredata_wheelbrakes"]
_OREM_COLS = ["tmstp_epoch", "temp_id", "msg_count", "sec_mark", "latitude", "longitude", "speed",
              "heading", "longitudinal_acceleration", "semi_major_axis_accuracy",
              "semi_minor_axis_accuracy", "wheel_brakes_flags"]
_WY_COLS = ["metadata_generatedat", "metadata_recordtype", "coredata_id", "coredata_secmark",
            "coredata_msgcnt", "coredata_position_lat", "coredata_position_long", "coredata_speed",
            "coredata_heading", "coredata_accuracy_semimajor", "coredata_accuracy_semiminor"]


def queries(orem_days=None):
    q = [
        {"name": "tampa", "dataset": TAMPA, "file": "tampa.csv", "select": _TAMPA_COLS,
         "order": ["coredata_id", "metadata_recordgeneratedat", ":id"]},
        {"name": "wyoming", "dataset": WYOMING, "file": "wyoming.csv", "select": _WY_COLS,
         "order": ["coredata_id", "metadata_generatedat", ":id"]},
    ]
    for d in (orem_days or OREM_DAYS):
        q.append({"name": "orem_" + d, "dataset": OREM, "file": "orem_%s.csv" % d, "select": _OREM_COLS,
                  "where": "date_utc = '%sT00:00:00.000'" % d,
                  "order": ["temp_id", "tmstp_epoch", ":id"]})
    q.append({"name": "nyc", "dataset": NYC, "file": "nyc.csv", "page": 100, "limit_rows": NYC_EVENTS,
              "select": [":id", "eventheader_eventtype", "roadtypebin", "bsmlist"], "order": [":id"]})
    return q

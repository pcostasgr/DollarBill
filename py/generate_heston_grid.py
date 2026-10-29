"""Regenerate the independent dated Heston regression fixture (requires QuantLib)."""
import json
from pathlib import Path

import QuantLib as ql

base = ql.Date(29, 10, 2026)
ql.Settings.instance().evaluationDate = base
day_count = ql.Actual365Fixed()
rate = ql.YieldTermStructureHandle(ql.FlatForward(base, 0.05, day_count))
dividend = ql.YieldTermStructureHandle(ql.FlatForward(base, 0, day_count))
rows = []
for stressed, params in [(False, (0.04, 2, 0.04, 0.3, -0.7)),
                         (True, (0.09, 1.5, 0.09, 1, -0.9))]:
    process = ql.HestonProcess(rate, dividend, ql.QuoteHandle(ql.SimpleQuote(100)), *params)
    engine = ql.AnalyticHestonEngine(ql.HestonModel(process), 1e-10, 100000)
    for days in [7, 30, 365]:
        for strike in [70, 85, 100, 115, 130]:
            option = ql.VanillaOption(ql.PlainVanillaPayoff(ql.Option.Call, strike),
                                      ql.EuropeanExercise(base + days))
            option.setPricingEngine(engine)
            rows.append(dict(stressed=stressed, days=days, strike=strike, call=option.NPV()))

fixture = dict(quantlib=ql.__version__, evaluation_date="2026-10-29",
               day_count="Actual365Fixed", rate=0.05, dividend_yield=0,
               engine="AnalyticHestonEngine adaptive integration, tolerance 1e-10", cases=rows)
destination = Path(__file__).resolve().parents[1] / "tests/fixtures/heston_quantlib_grid.json"
destination.parent.mkdir(exist_ok=True)
destination.write_text(json.dumps(fixture, indent=2) + "\n", encoding="utf-8")

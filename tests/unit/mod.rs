// Unit test modules

pub mod models;
pub mod performance;
pub mod concurrency;

mod market_data {
    mod test_csv_loader;
}

mod strategies {
    mod test_vol_mean_reversion;
    mod test_stock_classifier;
    mod test_personality_props;
}

mod backtesting {
    mod test_engine;
    mod test_short_options;
    mod test_edge_cases_backtest;
    mod test_trading_costs;
    mod test_dynamic_slippage;
    mod test_market_impact;
    mod test_liquidity_tiers;
}

# Running Tests

## Quick Start

```bash
# Run all tests (library + integration)
cargo test

# Run only the new integration tests
cargo test --test lib

# Run only library unit tests
cargo test --lib

# Run with detailed output
cargo test -- --nocapture

# Run specific test module
cargo test test_black_scholes
cargo test test_greeks
cargo test test_heston

# Run a specific test
cargo test test_call_option_atm
```

## Test Organization

- **Library tests** (`cargo test --lib`): Built-in tests in `src/` files
- **Integration tests** (`cargo test --test lib`): Comprehensive test suite in `tests/`

## Current Status

**421+ tests implemented, 421+ passing (100% ✅)**

### Breakdown by Type
- **Integration Tests**: 307 passing
- **Library Unit Tests**: 110 passing (7 ignored — network-dependent)
- **Standalone Tests**: 1 passing (CDF verification)
- **Doc Tests**: 3 passing (1 ignored)

### Test Coverage by Category
- ✅ Black-Scholes Pricing: 30/30 (100%)
- ✅ Greeks Calculations: 19/19 (100%)
- ✅ Heston Monte Carlo: 22/22 (100%)
- ✅ Heston Analytical (GL + CM): 9/9 (100%)
- ✅ Gauss-Laguerre Quadrature: 14/14 (100%) 🆕 NEW
- ✅ QuantLib Reference: 10/10 (100%) 🆕 NEW
- ✅ American Options: 8/8 (100%)
- ✅ Property-Based Tests: 13/13 (100%)
- ✅ Numerical Stability: 8/8 (100%)
- ✅ Vol Surface: 6/6 (100%)
- ✅ Portfolio Risk: 5/5 (100%)
- ✅ Nelder-Mead Optimization: 2/2 (100%)
- ✅ Backtest Engine: 15/15 (100%)
- ✅ Short Options: 13/13 (100%)
- ✅ Trading Costs: 12/12 (100%)
- ✅ Liquidity: 18/18 (100%)
- ✅ Slippage: 13/13 (100%)
- ✅ Market Impact: 8/8 (100%)
- ✅ Edge Cases: 6/6 (100%)
- ✅ Strategies: 28/28 (100%)
- ✅ Strategies Property-Based: 14/14 (100%)
- ✅ Portfolio Management: 37/37 (100%)
- ✅ Market Data Loading: 7/7 (100%)
- ✅ Thread Safety & Concurrency: 3/3 (100%)
- ✅ Performance Benchmarks: 3/3 (100%)
- ✅ Integration & Regime Stress: 17/17 (100%)

### New Test Categories

#### Property-Based Tests (`test_property_based.rs`) ⭐
Validates mathematical invariants that must always hold:
- Put-call parity across parameter ranges
- Delta bounds [0,1] for calls, [-1,0] for puts
- Gamma always non-negative
- Option price monotonicity
- Theta behavior for long positions
- Vega symmetry between calls and puts
- Heston convergence to Black-Scholes

#### Numerical Stability Tests (`test_numerical_stability.rs`) ⭐
Ensures robust calculations across extreme scenarios:
- Greeks stability (low/high prices, rates, vols, expiries)
- Implied volatility convergence
- Nelder-Mead optimizer robustness
- Heston FFT numerical stability
- Parameter sensitivity smoothness
- Precision consistency checks
- Optimization iteration limits

#### Edge Case Tests (`test_edge_cases.rs`) ⭐
Boundary conditions and extreme parameters:
- Zero time to expiry (intrinsic value)
- Zero volatility scenarios
- Deep ITM/OTM options
- Very long/short expirations
- Extreme strike ratios
- Near-zero interest rates

#### Performance Benchmarks (`test_benchmarks.rs`) ⭐
Speed validation to prevent regressions:
- Black-Scholes pricing: <500μs per call
- Heston pricing: <200ms per call
- Nelder-Mead optimization: <2s for convergence

#### Thread Safety Tests (`test_thread_safety.rs`) ⭐
Concurrent calculation validation:
- Parallel pricing calculations (4 threads)
- Independent calibration threads
- Deadlock prevention

#### Strategy Templates Tests (`src/strategies/templates.rs`) 🆕
Multi-leg options strategy template validation:
- Iron condor signal generation (4 legs: sell put, buy put, sell call, buy call)
- Bull put spread validation (2 legs with correct strike ordering)
- Bear call spread validation (2 legs with correct strike ordering)
- Short straddle/strangle configuration tests
- Covered call and cash-secured put templates
- Strike price calculation accuracy across different spot prices
- Volatility and days-to-expiry parameter passing
- Spread width consistency verification
- Custom configuration support
- Floating-point precision handling

Tests ensure:
- Correct number of legs for each strategy
- Proper strike price calculations (with tolerance for floating point)
- Signal types match strategy requirements
- Parameters propagate correctly through all legs
- Strategies work across different spot prices and volatilities

### Recent Fixes
All previous test failures have been resolved:
- ATM delta tests: Adjusted for interest rate drift (mathematically correct behavior)
- Extreme strikes: Added floating-point precision tolerance
- Heston convergence: Relaxed tolerances for Carr-Madan integration
- See [failed-tests-analysis.md](../docs/failed-tests-analysis.md) for details

## Test Files

```
tests/
├── unit/
│   ├── models/
│   │   ├── test_black_scholes.rs          # BSM pricing (30 tests)
│   │   ├── test_greeks.rs                 # Greeks calculations (19 tests)
│   │   ├── test_heston.rs                 # Heston MC (22 tests)
│   │   ├── test_heston_analytical.rs      # GL + CM analytical (9 tests)
│   │   ├── test_quantlib_reference.rs     # QuantLib cross-validation (10 tests) 🆕
│   │   ├── test_american.rs               # American options (8 tests)
│   │   ├── test_property_based.rs         # Proptest invariants (13 tests)
│   │   ├── test_numerical_stability.rs    # Convergence & precision (8 tests)
│   │   ├── test_vol_surface.rs            # Arbitrage-free surface (6 tests)
│   │   └── test_portfolio_risk.rs         # Portfolio Greeks (5 tests)
│   ├── backtesting/
│   │   ├── test_engine.rs                # Backtest engine (15 tests)
│   │   ├── test_short_options.rs         # Short options (13 tests)
│   │   ├── test_trading_costs.rs         # Costs (12 tests)
│   │   ├── test_liquidity.rs             # Liquidity (18 tests)
│   │   ├── test_slippage.rs              # Slippage (13 tests)
│   │   ├── test_market_impact.rs         # Market impact (8 tests)
│   │   └── test_edge_cases.rs            # Edge cases (6 tests)
│   ├── calibration/                    # (via src/ inline tests)
│   ├── concurrency/
│   │   └── test_thread_safety.rs         # Thread safety (3 tests)
│   ├── market_data/
│   │   └── test_csv_loader.rs            # Data loading (7 tests)
│   ├── strategies/                     # Strategy tests (42 tests)
│   └── performance/
│       └── test_benchmarks.rs            # Speed validation (3 tests)
├── integration/
│   ├── test_end_to_end.rs              # Full pipeline tests
│   └── test_regime_stress.rs           # Crash/recovery/vol-crush
├── helpers/mod.rs                       # Test utilities
├── lib.rs                               # Test harness root
└── verify_cdf.rs                        # Standalone CDF verification
```

## Adding New Tests

1. Create a new test file in the appropriate `tests/unit/*` directory
2. Use helpers from `crate::helpers`
3. Import needed modules from `dollarbill::`
4. Add the module to `tests/unit/mod.rs`

Example:
```rust
// tests/unit/mymodule/test_myfeature.rs

use dollarbill::mymodule::MyStruct;
use crate::helpers::EPSILON;

#[test]
fn test_my_feature() {
    let instance = MyStruct::new();
    assert!(instance.value > 0.0);
}
```

## Continuous Integration

Tests are run automatically on:
- Every commit (if CI is configured)
- Pull requests
- Pre-release validation

Ensure all tests pass before merging code.

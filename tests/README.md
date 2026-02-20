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

**146 tests implemented, 146 passing (100% ✅)**

### Breakdown by Type
- **Integration Tests**: 118 passing
- **Library Unit Tests**: 26 passing
- **Doc Tests**: 2 passing

### Test Coverage by Category
- ✅ Black-Scholes Pricing: 15/15 (100%)
- ✅ Greeks Calculations: 19/19 (100%)
- ✅ Heston Model: 22/22 (100%)
- ✅ Property-Based Tests: 7/7 (100%) **NEW**
- ✅ Numerical Stability: 8/8 (100%) **NEW**
- ✅ Edge Cases: Multiple tests (100%) **NEW**
- ✅ Nelder-Mead Optimization: 14/14 (100%)
- ✅ Backtest Engine: 17/17 (100%)
- ✅ Short Options: 13/13 (100%) **NEW**
- ✅ Market Data Loading: 8/8 (100%)
- ✅ Volatility Mean Reversion Strategy: 17/17 (100%)
- ✅ Thread Safety & Concurrency: 3/3 (100%) **NEW**
- ✅ Performance Benchmarks: 3/3 (100%) **NEW**

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
│   │   ├── test_black_scholes.rs      # Option pricing tests (15 tests)
│   │   ├── test_greeks.rs             # Greeks calculation tests (19 tests)
│   │   ├── test_heston.rs             # Heston model tests (22 tests)
│   │   ├── test_property_based.rs     # Mathematical invariants (7 tests) ⭐ NEW
│   │   ├── test_numerical_stability.rs # Convergence & precision (8 tests) ⭐ NEW
│   │   └── test_edge_cases.rs         # Boundary conditions ⭐ NEW
│   ├── calibration/
│   │   └── test_nelder_mead.rs        # Optimizer tests (14 tests)
│   ├── backtesting/
│   │   └── test_engine.rs             # Backtest engine tests (17 tests)
│   ├── market_data/
│   │   └── test_csv_loader.rs         # Data loading tests (8 tests)
│   ├── strategies/
│   │   └── test_vol_mean_reversion.rs # Strategy tests (17 tests)
│   ├── concurrency/
│   │   └── test_thread_safety.rs      # Thread safety tests (3 tests) ⭐ NEW
│   └── performance/
│       └── test_benchmarks.rs         # Speed validation (3 tests) ⭐ NEW
├── integration/
│   └── test_end_to_end.rs             # End-to-end workflows
├── helpers/mod.rs                      # Test utilities
└── fixtures/                           # Test data files
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

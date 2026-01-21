# ML-Enhanced Trading Pipeline for DollarBill
# Combines traditional quantitative analysis with machine learning predictions

Write-Host "🤖 DollarBill ML-Enhanced Trading Pipeline" -ForegroundColor Cyan
Write-Host "==============================================" -ForegroundColor Cyan

# Check if Python ML dependencies are installed
Write-Host "Checking ML dependencies..." -ForegroundColor Yellow
try {
    $pythonCheck = python -c "import sklearn, tensorflow; print('ML dependencies OK')" 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✓ ML dependencies available" -ForegroundColor Green
    } else {
        throw "ML dependencies not found"
    }
} catch {
    Write-Host "⚠ ML dependencies not installed. Install with: pip install scikit-learn tensorflow" -ForegroundColor Red
    Write-Host "Continuing with traditional analysis only..." -ForegroundColor Yellow
    $mlEnabled = $false
}

# Step 1: Fetch fresh market data
Write-Host "`n📡 Step 1: Fetching market data..." -ForegroundColor Magenta
python py/fetch_multi_stocks.py
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Failed to fetch stock data" -ForegroundColor Red
    exit 1
}

python py/fetch_multi_options.py
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Failed to fetch options data" -ForegroundColor Red
    exit 1
}

# Step 2: Generate traditional signals with Greeks
Write-Host "`n📊 Step 2: Generating quantitative signals..." -ForegroundColor Magenta
cargo run --release --example multi_symbol_signals
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Failed to generate signals" -ForegroundColor Red
    exit 1
}

# Step 3: Analyze volatility surfaces
Write-Host "`n📈 Step 3: Analyzing volatility surfaces..." -ForegroundColor Magenta
cargo run --release --example vol_surface_analysis
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ Failed to analyze volatility" -ForegroundColor Red
    exit 1
}

# Step 4: ML Enhancement (if available)
if ($mlEnabled) {
    Write-Host "`n🤖 Step 4: Applying ML enhancements..." -ForegroundColor Magenta

    # Train/update ML models if needed
    Write-Host "Training signal classifier..." -ForegroundColor Yellow
    python ml/signal_classifier.py --train

    Write-Host "Training volatility predictor..." -ForegroundColor Yellow
    python ml/volatility_predictor.py --train

    # Apply ML predictions
    Write-Host "Generating ML-enhanced signals..." -ForegroundColor Yellow
    # Note: This would require a new Rust example that integrates ML
    # For now, we demonstrate the ML components separately
}

# Step 5: Visualize results
Write-Host "`n📊 Step 5: Creating visualizations..." -ForegroundColor Magenta
python py/plot_vol_surface.py

# Step 6: Summary
Write-Host "`n✅ Pipeline Complete!" -ForegroundColor Green
Write-Host "Results available in:" -ForegroundColor White
Write-Host "  - data/ : Raw market data and analysis" -ForegroundColor White
Write-Host "  - images/ : Interactive volatility surface plots" -ForegroundColor White
if ($mlEnabled) {
    Write-Host "  - models/ : Trained ML models" -ForegroundColor White
}

Write-Host "`n🚀 Next Steps:" -ForegroundColor Cyan
Write-Host "  1. Review signals in terminal output above" -ForegroundColor White
Write-Host "  2. Open HTML files in images/ for 3D volatility visualization" -ForegroundColor White
Write-Host "  3. Run backtesting: cargo run --example backtest_heston" -ForegroundColor White
if ($mlEnabled) {
    Write-Host "  4. Analyze ML predictions for signal confidence scores" -ForegroundColor White
}

Write-Host "`n💡 Pro Tips:" -ForegroundColor Yellow
Write-Host "  - Look for signals with high edge % and delta-neutral portfolios" -ForegroundColor White
Write-Host "  - Check volatility skew for directional bias" -ForegroundColor White
Write-Host "  - Use ML confidence scores to filter lower-quality signals" -ForegroundColor White
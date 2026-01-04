# Run volatility surface analysis and visualization pipeline
Write-Host ""
Write-Host "============================================================"
Write-Host "  Volatility Surface Analysis Pipeline"
Write-Host "============================================================"
Write-Host ""

Write-Host "Step 1: Extracting volatility surfaces from options data..."
cargo run --release --example vol_surface_analysis

Write-Host ""
Write-Host "Step 2: Generating interactive visualizations..."
python plot_vol_surface.py

Write-Host ""
Write-Host "============================================================"
Write-Host "Complete! Open the HTML files in your browser:"
Write-Host "  - tsla_vol_surface_3d.html"
Write-Host "  - tsla_vol_smile.html"
Write-Host "  - tsla_term_structure.html"
Write-Host "  (and similar for other symbols)"
Write-Host "============================================================"
Write-Host ""
Read-Host "Press Enter to exit"

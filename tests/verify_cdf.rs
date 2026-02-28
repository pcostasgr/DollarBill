#[test]
fn verify_norm_cdf_values() {
    use dollarbill::models::bs_mod::norm_cdf_abst;
    // Known values of the standard normal CDF
    let cases = [
        (0.0, 0.5000),
        (0.5, 0.6915),
        (1.0, 0.8413),
        (1.5, 0.9332),
        (2.0, 0.9772),
        (-1.0, 0.1587),
    ];
    for (x, expected) in cases {
        let got = norm_cdf_abst(x);
        let err = (got - expected).abs();
        eprintln!("N({x:5.1}) = {got:.6}  expected {expected:.4}  err={err:.6}");
        assert!(err < 0.005, "N({x}) = {got} but expected ~{expected}, err={err}");
    }
}

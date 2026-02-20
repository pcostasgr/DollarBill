//! Concurrency and thread safety tests (lightweight)

use std::sync::Arc;
use std::thread;
use std::time::Instant;
use dollarbill::calibration::nelder_mead::{NelderMead, NelderMeadConfig};
use dollarbill::models::bs_mod::black_scholes_call;

#[test]
fn test_concurrent_pricing_calculations() {
    let num_threads = 4;
    let calculations_per_thread = 200;

    let handles: Vec<_> = (0..num_threads).map(|thread_id| {
        thread::spawn(move || {
            for i in 0..calculations_per_thread {
                let spot = 100.0 + (thread_id as f64) * 0.1;
                let strike = 95.0 + (i as f64) * 0.01;
                let _ = black_scholes_call(spot, strike, 0.25, 0.05, 0.2).price;
            }
        })
    }).collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

#[test]
fn test_parallel_calibration_independence() {
    let num_parallel_calibrations = 3;

    let handles: Vec<_> = (0..num_parallel_calibrations).map(|calib_id| {
        thread::spawn(move || {
            let target = 2.0 + calib_id as f64;
            let quadratic = |x: &[f64]| (x[0] - target).powi(2) + (x[1] - target).powi(2);
            let optimizer = NelderMead::new(NelderMeadConfig::default());
            let result = optimizer.minimize(&quadratic, vec![0.0, 0.0]);
            result.best_params
        })
    }).collect();

    for handle in handles {
        let params = handle.join().expect("Calibration thread panicked");
        assert!(params.len() == 2);
    }
}

#[test]
fn test_deadlock_prevention() {
    use std::sync::RwLock;

    let data1 = Arc::new(RwLock::new(Vec::<f64>::new()));
    let data2 = Arc::new(RwLock::new(Vec::<f64>::new()));

    let num_threads = 4;
    let operations_per_thread = 20;

    let handles: Vec<_> = (0..num_threads).map(|thread_id| {
        let d1 = Arc::clone(&data1);
        let d2 = Arc::clone(&data2);

        thread::spawn(move || {
            for i in 0..operations_per_thread {
                let value = black_scholes_call(100.0 + thread_id as f64 + i as f64 * 0.1, 100.0, 0.25, 0.05, 0.2).price;

                if thread_id % 2 == 0 {
                    let mut vec1 = d1.write().unwrap();
                    vec1.push(value);
                    drop(vec1);
                    let mut vec2 = d2.write().unwrap();
                    vec2.push(value * 2.0);
                    drop(vec2);
                } else {
                    let mut vec2 = d2.write().unwrap();
                    vec2.push(value * 2.0);
                    drop(vec2);
                    let mut vec1 = d1.write().unwrap();
                    vec1.push(value);
                    drop(vec1);
                }

                drop(d1.read().unwrap());
                drop(d2.read().unwrap());
            }
        })
    }).collect();

    let start_time = Instant::now();
    for handle in handles {
        handle.join().expect("Thread panicked or deadlocked");
    }
    let duration = start_time.elapsed();
    assert!(duration.as_secs() < 5, "Test took too long: {:.2?}", duration);

    let vec1 = data1.read().unwrap();
    let vec2 = data2.read().unwrap();
    assert_eq!(vec1.len(), num_threads * operations_per_thread);
    assert_eq!(vec2.len(), num_threads * operations_per_thread);
}

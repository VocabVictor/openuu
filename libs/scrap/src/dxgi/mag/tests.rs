use super::*;
#[test]
fn test() {
    let mut capture_mag = CapturerMag::new((0, 0), 1920, 1080).unwrap();
    capture_mag.exclude("", "RustDeskPrivacyWindow").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1000 * 10));
    let mut data = Vec::new();
    capture_mag.frame(&mut data).unwrap();
    println!("capture data len: {}", data.len());
}

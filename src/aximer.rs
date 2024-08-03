use alsa::mixer::{Mixer, SelemChannelId, SelemId};

fn get_volume(volume: i64) -> i64 {
    (volume as f64 * 65536. / 100.) as i64
}

fn get_volume_from_alsa(value: i64) -> i64 {
    (value as f64 * 100. / 65536.) as i64
}
pub fn get_left() -> Option<i64> {
    let mixer = Mixer::new("default", false).ok()?;

    // Find the master control
    let sid = SelemId::new("Master", 0);
    let selem = mixer
        .find_selem(&sid)
        .ok_or("Master control not found")
        .ok()?;

    // Define the left and right channel IDs
    let left = SelemChannelId::FrontLeft;

    Some(get_volume_from_alsa(selem.get_playback_volume(left).ok()?))
}

pub fn get_right() -> Option<i64> {
    let mixer = Mixer::new("default", false).ok()?;

    // Find the master control
    let sid = SelemId::new("Master", 0);
    let selem = mixer
        .find_selem(&sid)
        .ok_or("Master control not found")
        .ok()?;

    // Define the left and right channel IDs
    let right = SelemChannelId::FrontRight;

    Some(get_volume_from_alsa(selem.get_playback_volume(right).ok()?))
}
pub fn set_left(value: i64) -> Option<i64> {
    let mixer = Mixer::new("default", false).ok()?;

    // Find the master control
    let sid = SelemId::new("Master", 0);
    let selem = mixer
        .find_selem(&sid)
        .ok_or("Master control not found")
        .ok()?;

    // Define the left and right channel IDs
    let left = SelemChannelId::FrontLeft;

    // Set the volume for left and right channels (0 to 100)
    let left_volume = get_volume(value); // Adjust this value as needed
    selem.set_playback_volume(left, left_volume).ok()?;
    Some(get_volume_from_alsa(selem.get_playback_volume(left).ok()?))
}

pub fn set_right(value: i64) -> Option<i64> {
    let mixer = Mixer::new("default", false).ok()?;

    // Find the master control
    let sid = SelemId::new("Master", 0);
    let selem = mixer
        .find_selem(&sid)
        .ok_or("Master control not found")
        .ok()?;

    // Define the left and right channel IDs
    let right = SelemChannelId::FrontRight;

    // Set the volume for left and right channels (0 to 100)
    let right_volume = get_volume(value); // Adjust this value as needed
    selem.set_playback_volume(right, right_volume).ok()?;
    Some(get_volume_from_alsa(selem.get_playback_volume(right).ok()?))
}

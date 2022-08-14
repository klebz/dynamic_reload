use super::*;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Default)]
struct TestNotifyCallback {
    update_call_done: bool,
    after_update_done: bool,
    fail_update_done: bool,
}

impl TestNotifyCallback {
    fn update_call(&mut self, state: UpdateState, _lib: Option<&Arc<Lib>>) {
        match state {
            UpdateState::Before => self.update_call_done = true,
            UpdateState::After => self.after_update_done = true,
            UpdateState::ReloadFailed(_) => self.fail_update_done = true,
        }

        println!("Update state {:?}", self);
    }
}

fn get_test_shared_lib() -> PathBuf {
    let exe_path = env::current_exe().unwrap();
    let lib_path = exe_path.parent().unwrap().parent().unwrap();
    let lib_name = "test_shared";
    Path::new(&lib_path).join(DynamicReload::get_dynamiclib_name(lib_name))
}

#[test]
fn test_search_paths_none() {
    assert_eq!(DynamicReload::get_search_paths(None).len(), 0);
}

#[test]
fn test_search_paths_some() {
    assert_eq!(
        DynamicReload::get_search_paths(Some(vec!["test", "test"])).len(),
        2
    );
}

#[test]
fn test_get_watcher() {
    let (tx, _) = channel();
    // We expect this to always work
    assert!(DynamicReload::get_watcher(tx, Duration::from_secs(2)).is_some());
}

#[test]
fn test_get_temp_dir_fail() {
    assert!(DynamicReload::get_temp_dir(Some("_no_such_dir")).is_none());
}

#[test]
fn test_get_temp_dir_none() {
    assert!(DynamicReload::get_temp_dir(None).is_none());
}

#[test]
fn test_get_temp_dir_ok() {
    assert!(DynamicReload::get_temp_dir(Some("")).is_some());
}

#[test]
fn test_is_file_fail() {
    assert!(
        DynamicReload::is_file(&Path::new("haz_no_file_with_this_name").to_path_buf())
            .is_none()
    );
}

#[test]
fn test_is_file_ok() {
    assert!(DynamicReload::is_file(&env::current_exe().unwrap()).is_some());
}

#[test]
#[cfg(target_os = "macos")]
fn test_get_library_name_mac() {
    assert_eq!(
        DynamicReload::get_library_name("foobar", PlatformName::Yes),
        "libfoobar.dylib"
    );
}

#[test]
fn test_get_library_name() {
    assert_eq!(
        DynamicReload::get_library_name("foobar", PlatformName::No),
        "foobar"
    );
}

#[test]
fn test_search_backwards_from_file_ok() {
    // While this relays on having a Cargo project, it should be fine
    assert!(DynamicReload::search_backwards_from_exe(&"Cargo.toml".to_string()).is_some());
}

#[test]
fn test_search_backwards_from_file_fail() {
    assert!(DynamicReload::search_backwards_from_exe(&"_no_such_file".to_string()).is_none());
}

#[test]
fn test_add_library_fail() {
    let mut dr = DynamicReload::new(None, None, Search::Default, Duration::from_secs(2));
    unsafe {
        assert!(dr
            .add_library("wont_find_this_lib", PlatformName::No)
            .is_err());
    }
}

#[test]
fn test_add_shared_lib_ok() {
    let mut dr = DynamicReload::new(None, None, Search::Default, Duration::from_secs(2));
    unsafe {
        assert!(dr.add_library("test_shared", PlatformName::Yes).is_ok());
    }
}

#[test]
fn test_add_shared_lib_search_paths() {
    let mut dr = DynamicReload::new(
        Some(vec!["../..", "../test"]),
        None,
        Search::Default,
        Duration::from_secs(2),
    );
    unsafe {
        assert!(dr.add_library("test_shared", PlatformName::Yes).is_ok());
    }
}

#[test]
fn test_add_shared_lib_fail_load() {
    let mut dr = DynamicReload::new(None, None, Search::Default, Duration::from_secs(2));
    unsafe {
        assert!(dr.add_library("Cargo.toml", PlatformName::No).is_err());
    }
}

#[test]
fn test_add_shared_shadow_dir_ok() {
    let dr = DynamicReload::new(
        None,
        Some("target/debug"),
        Search::Default,
        Duration::from_secs(2),
    );
    assert!(dr.shadow_dir.is_some());
}

#[test]
fn test_add_shared_string_arg_ok() {
    let shadow_dir_string = "target/debug".to_owned();
    let dr = DynamicReload::new(
        None,
        Some(&shadow_dir_string),
        Search::Default,
        Duration::from_secs(2),
    );
    assert!(dr.shadow_dir.is_some());
}

#[test]
fn test_add_shared_lib_search_paths_strings() {
    let path1 = "../..".to_owned();
    let path2 = "../test".to_owned();
    let mut dr = DynamicReload::new(
        Some(vec![&path1, &path2]),
        None,
        Search::Default,
        Duration::from_secs(2),
    );
    unsafe {
        assert!(dr.add_library("test_shared", PlatformName::Yes).is_ok());
    }
}

#[test]
fn test_add_shared_update() {
    let mut notify_callback = TestNotifyCallback::default();
    let target_path = get_test_shared_lib();

    let mut dest_path = Path::new(&target_path).to_path_buf();

    let mut dr = DynamicReload::new(
        None,
        Some("target/debug"),
        Search::Default,
        Duration::from_secs(1),
    );

    dest_path.set_file_name("test_file");

    fs::copy(&target_path, &dest_path).unwrap();

    unsafe {
        assert!(dr.add_library("test_shared", PlatformName::Yes).is_ok());
    }

    for i in 0..10 {
        unsafe {
            dr.update(&TestNotifyCallback::update_call, &mut notify_callback);
        }

        if i == 2 {
            fs::copy(&dest_path, &target_path).unwrap();
        }

        thread::sleep(Duration::from_millis(200));
    }

    assert!(notify_callback.update_call_done);
    assert!(notify_callback.after_update_done);
}

#[test]
fn test_add_shared_update_fail_after() {
    let mut notify_callback = TestNotifyCallback::default();
    let target_path = get_test_shared_lib();
    let test_file = DynamicReload::get_dynamiclib_name("test_file_2");
    let mut dest_path = Path::new(&target_path).to_path_buf();

    let mut dr = DynamicReload::new(
        Some(vec!["target/debug"]),
        Some("target/debug"),
        Search::Default,
        Duration::from_secs(1),
    );

    assert!(dr.shadow_dir.is_some());

    dest_path.set_file_name(&test_file);

    DynamicReload::try_copy(&target_path, &dest_path).unwrap();

    // Wait a while before open the file. Not sure why this is needed.
    thread::sleep(Duration::from_millis(2000));

    unsafe {
        assert!(dr.add_library(&test_file, PlatformName::No).is_ok());
    }

    for i in 0..10 {
        println!("update {}", i);
        unsafe {
            dr.update(&TestNotifyCallback::update_call, &mut notify_callback);
        }

        if i == 2 {
            // Copy a non-shared lib to test the lib handles a broken "lib"
            fs::copy("Cargo.toml", &dest_path).unwrap();
        }

        thread::sleep(Duration::from_millis(200));
    }

    assert_eq!(notify_callback.update_call_done, true);
    assert_eq!(notify_callback.after_update_done, false);
    assert_eq!(notify_callback.fail_update_done, true);
}

#[test]
fn test_lib_equals_true() {
    let mut dr = DynamicReload::new(None, None, Search::Default, Duration::from_secs(2));
    let lib = unsafe { dr.add_library("test_shared", PlatformName::Yes).unwrap() };
    let lib2 = lib.clone();
    assert!(lib == lib2);
}

#[test]
fn test_lib_equals_false() {
    let mut dr = DynamicReload::new(
        Some(vec!["target/debug"]),
        Some("target/debug"),
        Search::Default,
        Duration::from_secs(2),
    );
    let target_path = get_test_shared_lib();

    let test_file = DynamicReload::get_dynamiclib_name("test_file_2");
    let mut dest_path = Path::new(&target_path).to_path_buf();

    dest_path.set_file_name(&test_file);

    let _ = DynamicReload::try_copy(&target_path, &dest_path);
    thread::sleep(Duration::from_millis(100));

    let lib0 = unsafe { dr.add_library(&test_file, PlatformName::No).unwrap() };
    let lib1 = unsafe { dr.add_library("test_shared", PlatformName::Yes).unwrap() };

    assert!(lib0 != lib1);
}

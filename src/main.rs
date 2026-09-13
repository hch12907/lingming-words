use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, DirEntry, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;

fn is_suitable_file(file: io::Result<DirEntry>, request: &'static str) -> Option<PathBuf> {
    let file = file.ok()?;
    let path = file.path();
    let is_yaml = path.extension().map(|ext| ext == "yaml").unwrap_or(false);
    let is_requested = path.file_stem().map(|stem| stem.to_string_lossy().contains(request)).unwrap_or(false);
    (is_yaml && is_requested && file.file_type().ok()?.is_file()).then(|| path)
}

fn read_file(path: PathBuf) -> (OsString, Vec<(String, String)>) {
    let name = path.file_stem().unwrap().to_owned();
    let mut file = File::open(&path).expect("无法打开文件");
    let mut content = String::new();
    file.read_to_string(&mut content).expect("无法读取文件");
    let content = content.split_once("...").expect("文件格式错误").1;
    let content = content.trim().lines().filter_map(|line| {
        let mut splitter = line.split('\t');
        let word = splitter.next()?.trim();
        let code = splitter.next()?.trim();
        Some((word.to_owned(), code.to_owned()))
    }).collect::<Vec<_>>();
    (name, content)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum 构词法 {
    // 传统五笔式构词法
    传统,

    // 取末构词法
    取末,

    // 取末构词法，但首字也取末
    双取末,

    // 跳声构词法
    跳声,

    // 取末兼跳声构词法
    取末兼跳声,
}

fn get_chaifen(chaifen: &'static str) -> HashMap<char, String> {
    let dir = fs::read_dir(".").expect("无法打开目录 `.`");
    dir
        .filter_map(|file| is_suitable_file(file, chaifen))
        .next()
        .map(read_file)
        .expect("未发现 chaifen 文件")
        .1
        .into_iter()
        .filter_map(|(zi, chai)| {
            if zi.chars().count() != 1 {
                println!("警告：{zi}的U码长度不为1！");
                return None
            }

            let zi = zi.chars().next().unwrap();

            let mut split = chai.split(',');
            let chaifen = split.nth(2).map(|chai| (zi, chai.to_ascii_lowercase()));
            let category = split.nth(2);
            if category.map(|c| c.contains("CJK")) == Some(true) {
                chaifen
            } else {
                None
            }
        })
        .collect::<HashMap<_, _>>()
}

fn make_word_file(
    path: String,
    chaifen: &HashMap<char, String>,
    words: &Vec<String>,
    strategy: 构词法,
    dry: bool,
) -> Vec<(String, String)> {
    let mut words_lastroot = Vec::new();

    for word in words {
        let mut word_chars = word.chars();
        let word_len = word.chars().count();

        let code = match (word_len, strategy) {
            (0, _) => unreachable!(),
            (1, _) => panic!("发现单字词：{word}"),
            // 双字词（由A、B两字组成）有三个可能的编码格式：
            // - A1、A2、B1、Bz (正常情况，首字取前两码，次字取首码与末根大码)
            // - A1、B1、B2、Bz（如果首字是小字根，首字取一码，次字取首码、次码、末根大码）
            // - A1、B1、Bz （如果首字与次字都是小字根）
            // 如果末根大码已取，依次取末根声码等。
            (2, 构词法::取末 | 构词法::双取末) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    let zi1_b = if strategy == 构词法::取末 {
                        zi1_chai.next().unwrap_or_default()
                    } else {
                        zi1_chai.last().unwrap_or_default()
                    };

                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    assert!(zi1_b.len() == 0 || zi1_b.len() >= 2);
                    
                    if zi1_a.len() == 2 {
                        if zi1_b.len() >= 2 {
                            // 首根大码、次根大码
                            String::from(&zi1_a[0..1]) + &zi1_b[0..1]
                        } else {
                            // 首根大码而已
                            String::from(&zi1_a[0..1])
                        }
                    } else {
                        if zi1_b.len() >= 2 && strategy == 构词法::双取末 {
                            // 首根大码与末根大码
                            String::from(&zi1_a[0..1]) + &zi1_b[0..1]
                        } else {
                            // 首根大码与声码
                            String::from(&zi1_a[0..2])
                        }
                    }
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    // 首字是小字根，次字要取三码
                    if zi1.len() == 1 {
                        let zi2_a = zi2_chai.next().unwrap();
                        let zi2_b = zi2_chai.next().unwrap_or_default();
                        let zi2_z = zi2_chai.last().unwrap_or_default();

                        assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                        assert!(zi2_b.len() == 0 || zi2_b.len() >= 2);
                        assert!(zi2_z.len() == 0 || zi2_z.len() >= 2);

                        if zi2_a.len() == 2 {
                            if zi2_b.len() >= 2 {
                                if zi2_z.len() >= 2 {
                                    // 首根大码、次根大码、末根大码
                                    String::from(&zi2_a[0..1]) + &zi2_b[0..1] + &zi2_z[0..1]
                                } else {
                                    // 首根大码、次根大码与次码
                                    String::from(&zi2_a[0..1]) + &zi2_b[0..2]
                                }
                            } else {
                                // 首根大码与韵码
                                String::from(&zi2_a[0..])
                            }
                        } else {
                            if zi2_z.len() >= 2 {
                                // 首根大码与末根大码
                                String::from(&zi2_a[0..2]) + &zi2_z[0..1]
                            } else if zi2_b.len() >= 2 {
                                // 首根大码与次根大码
                                String::from(&zi2_a[0..2]) + &zi2_b[0..1]
                            } else {
                                // 首根大码与首根声码
                                String::from(&zi2_a[0..3])
                            }
                        }
                    }
                    // 首字不是小字根，次字只取二码
                    else {
                        let zi2_a = zi2_chai.next().unwrap();
                        let zi2_z = zi2_chai.last().unwrap_or_default();

                        assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                        assert!(zi2_z.len() == 0 || zi2_z.len() >= 2);

                        if zi2_z.len() >= 2 {
                            // 首根大码与末根大码
                            String::from(&zi2_a[0..1]) + &zi2_z[0..1]
                        } else {
                            // 首根大码与次码
                            String::from(&zi2_a[0..2])
                        }
                    }
                };

                zi1 + &zi2
            },

            (3, 构词法::取末 | 构词法::双取末) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();
                let zi3 = word_chars.next().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    String::from(&zi1_a[0..1])
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    let zi2_a = zi2_chai.next().unwrap();
                    assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                    String::from(&zi2_a[0..1])
                };

                let zi3 = {
                    let mut zi3_chai = chaifen[&zi3].split('-');
                    let zi3_a = zi3_chai.next().unwrap();
                    let zi3_z = zi3_chai.last().unwrap_or_default();

                    assert!(zi3_a.len() == 2 || zi3_a.len() == 3);
                    assert!(zi3_z.len() == 0 || zi3_z.len() >= 2);

                    if zi3_z.len() >= 2 {
                        String::from(&zi3_a[0..1]) + &zi3_z[0..1]
                    } else {
                        String::from(&zi3_a[0..2])
                    }
                };

                zi1 + &zi2 + &zi3
            },

            (4.., 构词法::取末 | 构词法::双取末) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();
                let zi3 = word_chars.next().unwrap();
                let zi4 = word_chars.last().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    String::from(&zi1_a[0..1])
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    let zi2_a = zi2_chai.next().unwrap();
                    assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                    String::from(&zi2_a[0..1])
                };

                let zi3 = {
                    let mut zi3_chai = chaifen[&zi3].split('-');
                    let zi3_a = zi3_chai.next().unwrap();
                    assert!(zi3_a.len() == 2 || zi3_a.len() == 3);
                    String::from(&zi3_a[0..1])
                };

                let zi4 = {
                    let zi4_chai = chaifen[&zi4].split('-');
                    let zi4_z = zi4_chai.last().unwrap();
                    assert!(zi4_z.len() == 2 || zi4_z.len() == 3);
                    String::from(&zi4_z[0..1])
                };

                zi1 + &zi2 + &zi3 + &zi4
            },

            (2, 构词法::传统) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    let zi1_b = zi1_chai.next().unwrap_or_default();

                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    assert!(zi1_b.len() == 0 || zi1_b.len() >= 2);
                    
                    if zi1_a.len() == 2 {
                        if zi1_b.len() >= 2 {
                            // 首根大码、次根大码
                            String::from(&zi1_a[0..1]) + &zi1_b[0..1]
                        } else {
                            // 首根大码而已
                            String::from(&zi1_a[0..1])
                        }
                    } else {
                        // 首根大码与声码
                        String::from(&zi1_a[0..2])
                    }
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    // 首字是小字根，次字要取三码
                    if zi1.len() == 1 {
                        let zi2_a = zi2_chai.next().unwrap();
                        let zi2_b = zi2_chai.next().unwrap_or_default();
                        let zi2_c = zi2_chai.next().unwrap_or_default();

                        assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                        assert!(zi2_b.len() == 0 || zi2_b.len() >= 2);
                        assert!(zi2_c.len() == 0 || zi2_c.len() >= 2);

                        if zi2_a.len() == 2 {
                            if zi2_b.len() >= 2 {
                                if zi2_c.len() >= 2 {
                                    // 首根大码、次根大码、三根大码
                                    String::from(&zi2_a[0..1]) + &zi2_b[0..1] + &zi2_c[0..1]
                                } else {
                                    // 首根大码、次根大码与次码
                                    String::from(&zi2_a[0..1]) + &zi2_b[0..2]
                                }
                            } else {
                                // 首根大码与韵码
                                String::from(&zi2_a[0..])
                            }
                        } else {
                            if zi2_b.len() >= 2 {
                                // 首根大码、首根声码、次根大码
                                String::from(&zi2_a[0..2]) + &zi2_b[0..1]
                            } else {
                                // 首根大码、首根声码、首根韵码
                                String::from(&zi2_a[0..3])
                            }
                        }
                    }
                    // 首字不是小字根，次字只取二码
                    else {
                        let zi2_a = zi2_chai.next().unwrap();
                        let zi2_b = zi2_chai.next().unwrap_or_default();

                        assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                        assert!(zi2_b.len() == 0 || zi2_b.len() >= 2);

                        if zi2_a.len() == 3 || zi2_b.len() == 0 {
                            // 首根大码与次码
                            String::from(&zi2_a[0..2])
                        } else {
                            // 首根大码与次根大码
                            String::from(&zi2_a[0..1]) + &zi2_b[0..1]
                        }
                    }
                };

                zi1 + &zi2
            },

            (3, 构词法::传统) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();
                let zi3 = word_chars.next().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    String::from(&zi1_a[0..1])
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    let zi2_a = zi2_chai.next().unwrap();
                    assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                    String::from(&zi2_a[0..1])
                };

                let zi3 = {
                    let mut zi3_chai = chaifen[&zi3].split('-');
                    let zi3_a = zi3_chai.next().unwrap();
                    let zi3_b = zi3_chai.next().unwrap_or_default();

                    assert!(zi3_a.len() == 2 || zi3_a.len() == 3);
                    assert!(zi3_b.len() == 0 || zi3_b.len() >= 2);

                    if zi3_a.len() == 3 {
                        String::from(&zi3_a[0..2])
                    } else {
                        if zi3_b.len() >= 2 {
                            String::from(&zi3_a[0..1]) + &zi3_b[0..1]
                        } else {
                            String::from(&zi3_a[0..2])
                        }
                    }
                };

                zi1 + &zi2 + &zi3
            },

            (4.., 构词法::传统) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();
                let zi3 = word_chars.next().unwrap();
                let zi4 = word_chars.last().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    String::from(&zi1_a[0..1])
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    let zi2_a = zi2_chai.next().unwrap();
                    assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                    String::from(&zi2_a[0..1])
                };

                let zi3 = {
                    let mut zi3_chai = chaifen[&zi3].split('-');
                    let zi3_a = zi3_chai.next().unwrap();
                    assert!(zi3_a.len() == 2 || zi3_a.len() == 3);
                    String::from(&zi3_a[0..1])
                };

                let zi4 = {
                    let mut zi4_chai = chaifen[&zi4].split('-');
                    let zi4_a = zi4_chai.next().unwrap();
                    assert!(zi4_a.len() == 2 || zi4_a.len() == 3);
                    String::from(&zi4_a[0..1])
                };

                zi1 + &zi2 + &zi3 + &zi4
            },

            (2, 构词法::跳声) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    let zi1_b = zi1_chai.next().unwrap_or_default();

                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    assert!(zi1_b.len() == 0 || zi1_b.len() >= 2);

                    if zi1_b.len() >= 2 {
                        // 首根大码、次根大码
                        String::from(&zi1_a[0..1]) + &zi1_b[0..1]
                    } else {
                        if zi1_a.len() == 3 {
                            // 首根大码、首根声码
                            String::from(&zi1_a[0..2])
                        } else {
                            // 首根大码而已
                            String::from(&zi1_a[0..1])
                        }
                    }
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    // 首字是小字根，次字要取三码
                    if zi1.len() == 1 {
                        let zi2_a = zi2_chai.next().unwrap();
                        let zi2_b = zi2_chai.next().unwrap_or_default();
                        let zi2_c = zi2_chai.next().unwrap_or_default();

                        assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                        assert!(zi2_b.len() == 0 || zi2_b.len() >= 2);
                        assert!(zi2_c.len() == 0 || zi2_c.len() >= 2);

                        if zi2_b.len() >= 2 {
                            if zi2_c.len() >= 2 {
                                // 首根大码、次根大码、三根大码
                                String::from(&zi2_a[0..1]) + &zi2_b[0..1] + &zi2_c[0..1]
                            } else {
                                // 首根大码、次根大码与次码
                                String::from(&zi2_a[0..1]) + &zi2_b[0..2]
                            }
                        } else {
                            // 首根大码、声码、韵码
                            String::from(&zi2_a[0..])
                        }
                    }
                    // 首字不是小字根，次字只取二码
                    else {
                        let zi2_a = zi2_chai.next().unwrap();
                        let zi2_b = zi2_chai.next().unwrap_or_default();

                        assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                        assert!(zi2_b.len() == 0 || zi2_b.len() >= 2);

                        if zi2_b.len() >= 2 {
                            // 首根大码与次根大码
                            String::from(&zi2_a[0..1]) + &zi2_b[0..1]
                        } else {
                            // 首根大码与次码
                            String::from(&zi2_a[0..2])
                        }
                    }
                };

                zi1 + &zi2
            },

            (3, 构词法::跳声) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();
                let zi3 = word_chars.next().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    String::from(&zi1_a[0..1])
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    let zi2_a = zi2_chai.next().unwrap();
                    assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                    String::from(&zi2_a[0..1])
                };

                let zi3 = {
                    let mut zi3_chai = chaifen[&zi3].split('-');
                    let zi3_a = zi3_chai.next().unwrap();
                    let zi3_b = zi3_chai.next().unwrap_or_default();

                    assert!(zi3_a.len() == 2 || zi3_a.len() == 3);
                    assert!(zi3_b.len() == 0 || zi3_b.len() >= 2);

                    if zi3_b.len() >= 2 {
                        String::from(&zi3_a[0..1]) + &zi3_b[0..1]
                    } else {
                        String::from(&zi3_a[0..2])
                    }
                };

                zi1 + &zi2 + &zi3
            },

            (4.., 构词法::跳声) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();
                let zi3 = word_chars.next().unwrap();
                let zi4 = word_chars.last().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    String::from(&zi1_a[0..1])
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    let zi2_a = zi2_chai.next().unwrap();
                    assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                    String::from(&zi2_a[0..1])
                };

                let zi3 = {
                    let mut zi3_chai = chaifen[&zi3].split('-');
                    let zi3_a = zi3_chai.next().unwrap();
                    assert!(zi3_a.len() == 2 || zi3_a.len() == 3);
                    String::from(&zi3_a[0..1])
                };

                let zi4 = {
                    let mut zi4_chai = chaifen[&zi4].split('-');
                    let zi4_a = zi4_chai.next().unwrap();
                    assert!(zi4_a.len() == 2 || zi4_a.len() == 3);
                    String::from(&zi4_a[0..1])
                };

                zi1 + &zi2 + &zi3 + &zi4
            },

            (2, 构词法::取末兼跳声) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    let zi1_b = zi1_chai.next().unwrap_or_default();

                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    assert!(zi1_b.len() == 0 || zi1_b.len() >= 2);

                    if zi1_b.len() >= 2 {
                        // 首根大码、次根大码
                        String::from(&zi1_a[0..1]) + &zi1_b[0..1]
                    } else {
                        if zi1_a.len() == 3 {
                            // 首根大码、首根声码
                            String::from(&zi1_a[0..2])
                        } else {
                            // 首根大码而已
                            String::from(&zi1_a[0..1])
                        }
                    }
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    // 首字是小字根，次字要取三码
                    if zi1.len() == 1 {
                        let zi2_a = zi2_chai.next().unwrap();
                        let zi2_b = zi2_chai.next().unwrap_or_default();
                        let zi2_z = zi2_chai.last().unwrap_or_default();

                        assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                        assert!(zi2_b.len() == 0 || zi2_b.len() >= 2);
                        assert!(zi2_z.len() == 0 || zi2_z.len() >= 2);

                        if zi2_b.len() >= 2 {
                            if zi2_z.len() >= 2 {
                                // 首根大码、次根大码、末根大码
                                String::from(&zi2_a[0..1]) + &zi2_b[0..1] + &zi2_z[0..1]
                            } else {
                                // 首根大码、次根大码与次码
                                String::from(&zi2_a[0..1]) + &zi2_b[0..2]
                            }
                        } else {
                            // 首根大码、声码、韵码
                            String::from(&zi2_a[0..])
                        }
                    }
                    // 首字不是小字根，次字只取二码
                    else {
                        let zi2_a = zi2_chai.next().unwrap();
                        let zi2_z = zi2_chai.last().unwrap_or_default();

                        assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                        assert!(zi2_z.len() == 0 || zi2_z.len() >= 2);

                        if zi2_z.len() >= 2 {
                            // 首根大码与末根大码
                            String::from(&zi2_a[0..1]) + &zi2_z[0..1]
                        } else {
                            // 首根大码与次码
                            String::from(&zi2_a[0..2])
                        }
                    }
                };

                zi1 + &zi2
            },

            (3, 构词法::取末兼跳声) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();
                let zi3 = word_chars.next().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    String::from(&zi1_a[0..1])
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    let zi2_a = zi2_chai.next().unwrap();
                    assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                    String::from(&zi2_a[0..1])
                };

                let zi3 = {
                    let mut zi3_chai = chaifen[&zi3].split('-');
                    let zi3_a = zi3_chai.next().unwrap();
                    let zi3_z = zi3_chai.last().unwrap_or_default();

                    assert!(zi3_a.len() == 2 || zi3_a.len() == 3);
                    assert!(zi3_z.len() == 0 || zi3_z.len() >= 2);

                    if zi3_z.len() >= 2 {
                        String::from(&zi3_a[0..1]) + &zi3_z[0..1]
                    } else {
                        String::from(&zi3_a[0..2])
                    }
                };

                zi1 + &zi2 + &zi3
            },

            (4.., 构词法::取末兼跳声) => {
                let zi1 = word_chars.next().unwrap();
                let zi2 = word_chars.next().unwrap();
                let zi3 = word_chars.next().unwrap();
                let zi4 = word_chars.last().unwrap();

                let zi1 = {
                    let mut zi1_chai = chaifen[&zi1].split('-');
                    let zi1_a = zi1_chai.next().unwrap();
                    assert!(zi1_a.len() == 2 || zi1_a.len() == 3);
                    String::from(&zi1_a[0..1])
                };

                let zi2 = {
                    let mut zi2_chai = chaifen[&zi2].split('-');
                    let zi2_a = zi2_chai.next().unwrap();
                    assert!(zi2_a.len() == 2 || zi2_a.len() == 3);
                    String::from(&zi2_a[0..1])
                };

                let zi3 = {
                    let mut zi3_chai = chaifen[&zi3].split('-');
                    let zi3_a = zi3_chai.next().unwrap();
                    assert!(zi3_a.len() == 2 || zi3_a.len() == 3);
                    String::from(&zi3_a[0..1])
                };

                let zi4 = {
                    let zi4_chai = chaifen[&zi4].split('-');
                    let zi4_z = zi4_chai.last().unwrap();
                    assert!(zi4_z.len() == 2 || zi4_z.len() == 3);
                    String::from(&zi4_z[0..1])
                };

                zi1 + &zi2 + &zi3 + &zi4
            },
        };

        words_lastroot.push((word.to_owned(), code));
    }

    if !dry {
            let header =
format!(r#"# encoding: utf-8
---
name: "{}"
version: "20260705"
sort: original
columns:
  - text
  - code
...

"#, path.split('/').last().unwrap().replace(".dict.yaml", ""));
        let words_lastroot = words_lastroot.into_iter().map(|(word, code)| word + "\t" + &code).collect::<Vec<_>>();
        let words_lastroot = header + &words_lastroot.join("\n");

        File::create(path)
            .expect("无法创建新文件")
            .write_all(words_lastroot.as_bytes())
            .expect("无法写入新文件");

        Vec::new()
    } else {
        words_lastroot
    }
}

fn main() {
    // 获取拆分信息
    // chaifen/chaifen_tw的键是汉字，对应的值是拆分（一二三末，字根全码，使用短横线分隔，如：“bli-ke-ka”）
    let chaifen = get_chaifen("chaifen.dict");
    // let chaifen_tw = get_chaifen("chaifen_tw.dict");

    // 获取词库。
    // word_files是一个 (词库名, [单词]) 阵列
    let get_words = |words| {
        let dir = fs::read_dir(".").expect("无法打开目录 `.`");
    
        dir
            .filter_map(|file| is_suitable_file(file, words))
            .map(read_file)
            .map(|(name, words)| {
                (name, words.into_iter().map(|(word, _)| word).collect::<Vec<_>>())
            })
            .next()
            .expect(&format!("无法读取词库文件 {words}"))
    };
    let (sc_path, sc_words) = get_words("_sc.words.dict");
    let (tc_path, tc_words) = get_words("_tc.words.dict");

    make_word_file(
        sc_path.to_string_lossy().replace(".words", ".lastroot.words") + ".yaml",
        &chaifen,
        &sc_words,
        构词法::取末,
        false
    );
    make_word_file(
        tc_path.to_string_lossy().replace(".words", ".lastroot.words") + ".yaml",
        &chaifen,
        &tc_words,
        构词法::取末,
        false
    );
    make_word_file(
        sc_path.to_string_lossy().replace(".words", ".simple.words") + ".yaml",
        &chaifen,
        &sc_words,
        构词法::传统,
        false
    );
    make_word_file(
        tc_path.to_string_lossy().replace(".words", ".simple.words") + ".yaml",
        &chaifen,
        &tc_words,
        构词法::传统,
        false
    );
    make_word_file(
        sc_path.to_string_lossy().replace(".words", ".official.words") + ".yaml",
        &chaifen,
        &sc_words,
        构词法::跳声,
        false
    );
    make_word_file(
        tc_path.to_string_lossy().replace(".words", ".official.words") + ".yaml",
        &chaifen,
        &tc_words,
        构词法::跳声,
        false
    );
    make_word_file(
        sc_path.to_string_lossy().replace(".words", ".lastrootplus.words") + ".yaml",
        &chaifen,
        &sc_words,
        构词法::取末兼跳声,
        false
    );
    make_word_file(
        tc_path.to_string_lossy().replace(".words", ".lastrootplus.words") + ".yaml",
        &chaifen,
        &tc_words,
        构词法::取末兼跳声,
        false
    );
    make_word_file(
        sc_path.to_string_lossy().replace(".words", ".lastrootdouble.words") + ".yaml",
        &chaifen,
        &sc_words,
        构词法::双取末,
        false
    );
    make_word_file(
        tc_path.to_string_lossy().replace(".words", ".lastrootdouble.words") + ".yaml",
        &chaifen,
        &tc_words,
        构词法::双取末,
        false
    );
}

#[test]
fn test() {
    let chaifen = get_chaifen("chaifen.dict");

    // 取末、传统、省声、省声兼取末
    let expected_results = [
        ("一一",     ["ffi", "ffi", "ffi", "ffi"]), // 二字词：小字根+小字根
        ("正在",     ["fmvs", "fmvk", "fmks", "fmks"]), // 二字词：小字根+大字根
        ("其一",     ["qkfi", "qkfi", "qkfi", "qkfi"]), // 二字词：大字根+小字根
        ("大致",     ["ydkj", "ydkv", "ydkj", "ydkj"]), // 二字词：大字根+大字根
        ("培养基",   ["scqs", "scqk", "scqs", "scqs"]), // 三字词：小字根+小字根+小字根
        ("镇静剂",   ["hshv", "hshw", "hshc", "hshv"]), // 三字词：小字根+小字根+大字根
        ("一大早",   ["fyda", "fyda", "fyda", "fyda"]), // 三字词：小字根+大字根+小字根
        ("好日子",   ["fhhv", "fhhv", "fhhv", "fhhv"]), // 三字词：小字根+大字根+大字根
        ("非洲人",   ["yvne", "yvne", "yvne", "yvne"]), // 三字词：大字根+小字根+小字根
        ("大人物",   ["ynsk", "ynsn", "ynsp", "ynsk"]), // 三字词：大字根+小字根+大字根
        ("超导体",   ["hcjf", "hcjx", "hcjx", "hcjf"]), // 三字词：大字根+大字根+小字根
        ("非常多",   ["ykll", "yklx", "ykll", "ykll"]), // 三字词：大字根+大字根+大字根
        ("行色匆匆", ["kbpt", "kbpp", "kbpp", "kbpt"]), // 四字词：小字根+小字根+小字根+小字根
        ("以泪洗面", ["wvvd", "wvvd", "wvvd", "wvvd"]), // 四字词：小字根+小字根+小字根+大字根
        ("喃喃自语", ["ddnd", "ddnw", "ddnw", "ddnd"]), // 四字词：小字根+小字根+大字根+小字根
        ("鸡飞狗跳", ["qwwr", "qwwf", "qwwf", "qwwr"]), // 四字词：小字根+小字根+大字根+大字根
        ("生产工具", ["jbgl", "jbgv", "jbgv", "jbgl"]), // 四字词：小字根+大字根+小字根+小字根
        ("百废待兴", ["fhkl", "fhkn", "fhkn", "fhkl"]), // 四字词：小字根+大字根+小字根+大字根
        ("急不可耐", ["blhj", "blhd", "blhd", "blhj"]), // 四字词：小字根+大字根+大字根+小字根
        ("人高马大", ["ncry", "ncry", "ncry", "ncry"]), // 四字词：小字根+大字根+大字根+大字根
        ("星火燎原", ["hppn", "hppf", "hppf", "hppn"]), // 四字词：大字根+小字根+小字根+小字根
        ("心想事成", ["mxjw", "mxjm", "mxjm", "mxjw"]), // 四字词：大字根+小字根+小字根+大字根
        ("转来转去", ["mfmg", "mfms", "mfms", "mfmg"]), // 四字词：大字根+小字根+大字根+小字根
        ("连续不断", ["mcld", "mclv", "mclv", "mcld"]), // 四字词：大字根+小字根+大字根+大字根
        ("基础理论", ["qpyt", "qpyw", "qpyw", "qpyt"]), // 四字词：大字根+大字根+小字根+小字根
        ("点到为止", ["rktl", "rktl", "rktl", "rktl"]), // 四字词：大字根+大字根+小字根+大字根
        ("动手动脚", ["rlrj", "rlrl", "rlrl", "rlrj"]), // 四字词：大字根+大字根+大字根+小字根
        ("暴风骤雨", ["hkrd", "hkrd", "hkrd", "hkrd"]), // 四字词：大字根+大字根+大字根+大字根

        // 随便挑几个
        ("其中",   ["qkdk", "qkdk", "qkdk", "qkdk"]),
        ("正门",   ["fbme", "fbme", "fbme", "fbme"]),
        ("一种",   ["fwhk", "fwhd", "fwdk", "fwdk"]),
        ("风挡",   ["kjtb", "kjtn", "kbtn", "kbtb"]),
        ("侍者",   ["jssh", "jssl", "jssh", "jssh"]),
        ("成材",   ["mwxk", "mwxj", "mwxj", "mwxk"]),
        ("立地",   ["blss", "blss", "blss", "blss"]),
        ("进位",   ["rjjb", "rjjb", "rcjb", "rcjb"]),
        ("应考",   ["hgsr", "hgsl", "hnsr", "hnsr"]),
        ("凑数",   ["rjvj", "rjvm", "ryvf", "ryvj"]),
        ("手提箱",  ["ltnd", "ltnx", "ltnx", "ltnd"]),
        ("做手脚",  ["jllj", "jlls", "jlls", "jllj"]),
        ("修改版",  ["jcwq", "jcwp", "jcwr", "jcwq"]),
    ];

    let words = expected_results.iter().map(|(word, _)| (*word).to_owned()).collect::<Vec<_>>();

    let result0 = make_word_file(String::new(), &chaifen, &words, 构词法::取末, true);
    let result1 = make_word_file(String::new(), &chaifen, &words, 构词法::传统, true);
    let result2 = make_word_file(String::new(), &chaifen, &words, 构词法::跳声, true);
    let result3 = make_word_file(String::new(), &chaifen, &words, 构词法::取末兼跳声, true);

    for (i, (_word, expected)) in expected_results.iter().enumerate() {
        assert_eq!(result0[i].1, expected[0]);
        assert_eq!(result1[i].1, expected[1]);
        assert_eq!(result2[i].1, expected[2]);
        assert_eq!(result3[i].1, expected[3]);
    }
}

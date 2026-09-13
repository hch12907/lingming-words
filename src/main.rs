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

#[derive(Clone, Copy, Debug)]
enum 构词法 {
    // 传统五笔式构词法
    传统,

    // 取末构词法
    取末,

    // 跳声构词法
    跳声,

    // 取末兼跳声构词法
    取末兼跳声,
}

fn main() {
    // 获取拆分信息
    // chaifen/chaifen_tw的键是汉字，对应的值是拆分（一二三末，字根全码，使用短横线分隔，如：“bli-ke-ka”）
    let get_chaifen = |chaifen| {
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
    };
    let chaifen = get_chaifen("chaifen.dict");
    let chaifen_tw = get_chaifen("chaifen_tw.dict");

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

    // 计算新词库。
    let make_word_file = |path: String, words: &Vec<String>, is_tc: bool, strategy: 构词法| {
        let mut words_lastroot = Vec::new();

        let chaifen = if !is_tc { &chaifen } else { &chaifen };

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
                (2, 构词法::取末) => {
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
                                    String::from(&zi2_a[0..1]) + &zi2_z[0..1]
                                } else if zi2_b.len() >= 2 {
                                    // 首根大码与次根大码
                                    String::from(&zi2_a[0..1]) + &zi2_b[0..1]
                                } else {
                                    // 首根大码与首根声码
                                    String::from(&zi2_a[0..2])
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

                (3, 构词法::取末) => {
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

                (4.., 构词法::取末) => {
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

            words_lastroot.push(word.to_owned() + "\t" + &code);
        }

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
        let words_lastroot = header + &words_lastroot.join("\n");

        File::create(path)
            .expect("无法创建新文件")
            .write_all(words_lastroot.as_bytes())
            .expect("无法写入新文件")
    };

    make_word_file(
        sc_path.to_string_lossy().replace(".words", ".lastroot.words") + ".yaml",
        &sc_words,
        false,
        构词法::取末
    );
    make_word_file(
        tc_path.to_string_lossy().replace(".words", ".lastroot.words") + ".yaml",
        &tc_words,
        true,
        构词法::取末
    );
    make_word_file(
        sc_path.to_string_lossy().replace(".words", ".simple.words") + ".yaml",
        &sc_words,
        false,
        构词法::传统
    );
    make_word_file(
        tc_path.to_string_lossy().replace(".words", ".simple.words") + ".yaml",
        &tc_words,
        true,
        构词法::传统
    );
    make_word_file(
        sc_path.to_string_lossy().replace(".words", ".official.words") + ".yaml",
        &sc_words,
        false,
        构词法::跳声
    );
    make_word_file(
        tc_path.to_string_lossy().replace(".words", ".official.words") + ".yaml",
        &tc_words,
        true,
        构词法::跳声
    );
    make_word_file(
        sc_path.to_string_lossy().replace(".words", ".lastrootplus.words") + ".yaml",
        &sc_words,
        false,
        构词法::取末兼跳声
    );
    make_word_file(
        tc_path.to_string_lossy().replace(".words", ".lastrootplus.words") + ".yaml",
        &tc_words,
        true,
        构词法::取末兼跳声
    );
}

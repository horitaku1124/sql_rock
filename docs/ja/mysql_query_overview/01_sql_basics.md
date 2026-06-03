# MySQL クエリ仕様まとめ

## 1. SQLの基本構文

MySQLはSQL（Structured Query Language）を使用してデータベースを操作します。

主な分類：

| 分類 | 内容 |
|--------|--------|
| DDL | テーブルやデータベース定義 |
| DML | データ操作 |
| DQL | データ検索 |
| DCL | 権限管理 |
| TCL | トランザクション制御 |

---

## 1.1 データ型

MySQL のデータ型は、数値型、日付・時刻型、文字列・バイト列型、空間型、`JSON` 型に大別される。

### 1.1.1 型指定で使う記号

| 記号 | 内容 |
|---|---|
| `M` | 整数型では最大表示幅、浮動小数点型・固定小数点型では格納できる数字の総桁数、文字列型では最大長を表す。整数型の表示幅は値の範囲を制限しない。 |
| `D` | 浮動小数点型・固定小数点型で、小数点以下の桁数を表す。 |
| `fsp` | `TIME`、`DATETIME`、`TIMESTAMP` で秒の小数部の桁数を表す。指定可能な値は `0` から `6` で、省略時は `0`。 |

数値型には、符号なしの値だけを扱う `UNSIGNED` 属性を指定できる。`ZEROFILL` 属性と整数型の表示幅は非推奨である。

### 1.1.2 数値型

#### 整数型

整数を正確に格納する。必要な値の範囲に応じて型を選択する。

| 型 | 内容 |
|---|---|
| `TINYINT` | 小さい範囲の整数 |
| `SMALLINT` | `TINYINT` より広い範囲の整数 |
| `MEDIUMINT` | 中程度の範囲の整数 |
| `INT`, `INTEGER` | 一般的な整数。`INT` と `INTEGER` は同義 |
| `BIGINT` | 大きい範囲の整数 |

#### 固定小数点型

| 型 | 内容 |
|---|---|
| `DECIMAL(M,D)`, `NUMERIC(M,D)` | 小数を正確に格納する。金額など、丸め誤差を避けたい値に適する。`DEC` と `FIXED` も `DECIMAL` の同義語。 |

`M` と `D` を省略した場合、`DECIMAL` のデフォルトは `DECIMAL(10,0)` となる。

#### 浮動小数点型

| 型 | 内容 |
|---|---|
| `FLOAT` | 単精度の近似値 |
| `DOUBLE`, `DOUBLE PRECISION` | 倍精度の近似値。`DOUBLE` と `DOUBLE PRECISION` は同義 |
| `REAL` | 通常は `DOUBLE PRECISION` の同義。`REAL_AS_FLOAT` SQL モードでは `FLOAT` の同義 |

#### ビット値型

| 型 | 内容 |
|---|---|
| `BIT(M)` | ビット値を格納する。`M` は `1` から `64` で、省略時は `1`。 |

### 1.1.3 日付・時刻型

| 型 | 内容 |
|---|---|
| `DATE` | 日付 |
| `TIME(fsp)` | 時刻または経過時間 |
| `DATETIME(fsp)` | 日付と時刻 |
| `TIMESTAMP(fsp)` | タイムスタンプ。`DATETIME` と同様に自動初期化・自動更新を設定できる。 |
| `YEAR` | 年 |

入力値には複数の形式を利用できるが、日付部分は年、月、日の順で指定する。無効な値を扱う際の挙動は SQL モードにも依存する。

### 1.1.4 文字列・バイト列型

| 型 | 内容 |
|---|---|
| `CHAR(M)` | 固定長の文字列 |
| `VARCHAR(M)` | 可変長の文字列 |
| `BINARY(M)` | 固定長のバイト列 |
| `VARBINARY(M)` | 可変長のバイト列 |
| `TINYBLOB`, `BLOB`, `MEDIUMBLOB`, `LONGBLOB` | バイナリデータ。型ごとに格納可能な最大長が異なる。 |
| `TINYTEXT`, `TEXT`, `MEDIUMTEXT`, `LONGTEXT` | テキストデータ。型ごとに格納可能な最大長が異なる。 |
| `ENUM` | 定義済みの候補から 1 つの値を格納する。 |
| `SET` | 定義済みの候補から 0 個以上の値を組み合わせて格納する。 |

非バイナリ文字列の長さは文字数、バイナリ文字列の長さはバイト数として扱う。文字列型には文字セットを指定する `CHARACTER SET` と、照合順序を指定する `COLLATE` を設定できる。

#### VECTOR 型

| 型 | 内容 |
|---|---|
| `VECTOR(N)` | `N` 個の単精度浮動小数点値を格納するベクトル。`N` のデフォルトは `2048`、最大値は `16383`。 |

`VECTOR` は主キー、外部キー、ユニークキー、パーティションキーには利用できない。別の `VECTOR` との等価比較は可能だが、それ以外の比較には利用できない。

### 1.1.5 空間型

空間型は OpenGIS のクラスに対応し、位置や形状を表す値を格納する。

| 分類 | 型 | 内容 |
|---|---|---|
| 単一値 | `GEOMETRY` | 任意の種類の空間値 |
| 単一値 | `POINT` | 点 |
| 単一値 | `LINESTRING` | 線 |
| 単一値 | `POLYGON` | 多角形 |
| コレクション | `MULTIPOINT` | 複数の点 |
| コレクション | `MULTILINESTRING` | 複数の線 |
| コレクション | `MULTIPOLYGON` | 複数の多角形 |
| コレクション | `GEOMETRYCOLLECTION` | 任意の種類の空間値のコレクション |

空間型のカラムには、格納する値の空間参照系を制限する `SRID` 属性を設定できる。`SPATIAL` インデックスを利用する場合は、特定の `SRID` と `NOT NULL` を指定する。

### 1.1.6 JSON 型

| 型 | 内容 |
|---|---|
| `JSON` | JSON ドキュメントを格納する。 |

`JSON` 型は、格納時にドキュメントを検証し、不正な JSON をエラーにする。値は要素へ効率よくアクセスできる内部形式に変換される。

`JSON` カラム自体には直接インデックスを作成できない。JSON からスカラー値を取り出す生成カラムにインデックスを作成するか、JSON 配列に対するマルチバリューインデックスを利用する。

### 1.1.7 型選択の指針

- 格納する値を正確に表現できる、最小限の大きさの型を選択する。
- 固定長と可変長のどちらを使うかは、データの性質と格納効率を踏まえて選択する。
- `NULL` が不要なカラムには `NOT NULL` を指定する。
- 型の選択による性能差は、実際のデータと利用環境で検証する。

### 1.1.8 参考資料

- [MySQL 9.7 Reference Manual: Data Types](https://dev.mysql.com/doc/refman/9.7/en/data-types.html)
- [Numeric Data Types](https://dev.mysql.com/doc/refman/9.7/en/numeric-types.html)
- [Date and Time Data Types](https://dev.mysql.com/doc/refman/9.7/en/date-and-time-types.html)
- [String Data Types](https://dev.mysql.com/doc/refman/9.7/en/string-types.html)
- [The VECTOR Type](https://dev.mysql.com/doc/refman/9.7/en/vector.html)
- [Spatial Data Types](https://dev.mysql.com/doc/refman/9.7/en/spatial-type-overview.html)
- [The JSON Data Type](https://dev.mysql.com/doc/refman/9.7/en/json.html)

---

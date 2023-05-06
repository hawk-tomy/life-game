# 周辺マスのインデックス計算
## 空間の実装
左上を原点、左右方向をx、上下方向をyとしたとき、
空間全体を(x_length x y_length)の長さの配列で表現し、
(x, y)地点が生きているかを、`x + y*y_length`のbool値で表現する。

## 計算式
```
中心地点: idx
幅: width
高さ: height

場所の総数=配列の長さ: size = width * height
idxが左端か: idx % width == 0
idxが右端か: idx % width == width - 1

idxが左端の時の重み: right_weight = width
idxが左端でないとき: right_weight = 0
idxが右端の時の重み: left_weight = width
idxが右端でないとき: left_weight = 0

周辺マス: idx := cmの真上から時計回りに、cu, ru, rm, rd, cd, ld, lm, lu
```
すると、
```rs
cu = (size + idx - width) % size
cd = (size + idx + width) % size

ru = (size + idx + 1 - right_weight - width) % size
rm = (size + idx + 1 - right_weight) % size
rd = (size + idx + 1 - right_weight + width) % size

lu = (size + idx - 1 + left_weight - width) % size
lm = (size + idx - 1 + left_weight) % size
ld = (size + idx - 1 + left_weight + width) % size
```
## 導出
このライフゲームはトーラスの上に乗っている(右端から右は左端で一番下から下は一番上に行く)ことになっているため
3x3のボードのidxの並びは、疑似的には以下のようになる(ただし、idxは常に中央の3x3上)
```
0 1 2 | 0 1 2 | 0 1 2 
3 4 5 | 3 4 5 | 3 4 5 
6 7 8 | 6 7 8 | 6 7 8 
------+-------+------
0 1 2 | 0 1 2 | 0 1 2 
3 4 5 | 3 4 5 | 3 4 5 
6 7 8 | 6 7 8 | 6 7 8 
------+-------+------
0 1 2 | 0 1 2 | 0 1 2 
3 4 5 | 3 4 5 | 3 4 5 
6 7 8 | 6 7 8 | 6 7 8 
```
まず、中央であるidx=4について考える。
この時、上下は、idx-width, idx+width(一列分ずれる)。
また、それぞれの左右は、idx-1, idx+1。
よって、
```rs
cu = idx - width
cd = idx + width

ru = idx + 1 - width
rm = idx + 1
rd = idx + 1 + width

lu = idx - 1 - width
lm = idx - 1
ld = idx - 1 + width
```
idx=7のとき、図より、周囲(真上より時計回り。以下同)は4 5 8 2 1 0 6 3。
計算式に基づくと、4 5 8 11 10 9 6 3となる。
ここで、size-1の次は配列の最初に戻らなければならないが、これはsizeのmodと同一視できる。
また、0 <= x < sizeのとき、x = x % sizeであるため、とりあえずsizeでmodを取ればよい。
よって、
```rs
cu = (idx - width) % size
cd = (idx + width) % size

ru = (idx + 1 - width) % size
rm = (idx + 1) % size
rd = (idx + 1 + width) % size

lu = (idx - 1 - width) % size
lm = (idx - 1) % size
ld = (idx - 1 + width) % size
```
idx=1のとき、図より周囲は7 8 2 5 4 3 0 6。
計算式に基づくと、-2 -1 2 5 4 3 0 -3となる。
ここで、0のひとつ前は配列の最後に移動しなければならないため、-1とはsize-1である。
前項より、計算後に常にsizeのmodをとっているため、先に無条件にsizeを足しておくことでuintの範囲で計算ができる。
よって、
```rs
cu = (size + idx - width) % size
cd = (size + idx + width) % size

ru = (size + idx + 1 - width) % size
rm = (size + idx + 1) % size
rd = (size + idx + 1 + width) % size

lu = (size + idx - 1 - width) % size
lm = (size + idx - 1) % size
ld = (size + idx - 1 + width) % size
```
idx=3のとき、図より、周囲は0 1 4 7 6 8 5 2。
上述の計算式に基づくと、0 1 4 7 6 5 2 8となる。
本来自身が左端の時、自身の左はひとつ前ではなくて、その行の右端でなければならない。
ここで、ひとつ前とは直前の行の右端であるから、その一列下、
つまり、自身が左端ならば、左端の算出時には幅を加算する必要がある。
同様にidx=5のとき、周囲は2 0 3 6 8 7 4 1だが、2 3 6 0 8 7 4 1となるため、
自身が右端ならば、右端の算出時には幅を減算する必要がある。
よって、
```rs
left_weight = if idx % width == 0 { width } else { 0 }
right_weight = if idx % width == width - 1 { width } else { 0 }

cu = (size + idx - width) % size
cd = (size + idx + width) % size

ru = (size + idx + 1 - right_weight - width) % size
rm = (size + idx + 1 - right_weight) % size
rd = (size + idx + 1 - right_weight + width) % size

lu = (size + idx - 1 + left_weight - width) % size
lm = (size + idx - 1 + left_weight) % size
ld = (size + idx - 1 + left_weight + width) % size
```
なお、idx=0,2,6,8について確認すると、計算式は成立している。

次に上限を確認する。
c/rust等でuint(桁数不問)を用いた場合に計算中にuintの範囲を超えないsizeの最大値を考える。

全体として最も減算量が多いのは、idx=width-1(右上)の右上のときで、この値はsize-width。
w=2,h=1,s=2の
```
0 1
```
では、ru = (2 + 1 + 1 - 2  - 2) % 2より、括弧内は0となる。
これは、idxが右端の時に右に移動した時の正しい位置がその行の左端であることから(idx + 1 - right_weight)=0
idxが上端の時に上に移動したときの正しい位置の算出の為に先にsizeを加算していることによる。
よって計算時には常に0を下回ることはない。
逆に加算量が多いのは、idx=size-width(左下)の左下のときで、この値はwidth-1。
同様の例によれば、ld = (2 + 0 - 1 + 2 + 2) % 2より、括弧内は5となる。
つまり、3*max(size)-1 <= max(uint)を満たさなければならない。
max(uint) = 2^n - 1 とすると、max(size) <= 2^n/3となる。
u16であれば、max(size) <= 21845となる。
また、u32であれば、max(size) <= 1431655765となる。

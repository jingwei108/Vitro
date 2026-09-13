//go:build windows

// Package pyrandom 是 CPython random 模块（MT19937）的 Go 逐比特复刻。
//
// 背景：D5 第三/四站为双轨对账复刻 Python 探针，pyRandom 整份实现在
// random_diff 与 interaction_probe 各写一份（~200 行）。RNG 漂移 = 可复现
// 基线失效（同 seed 必须生成同一用例集合），故单源化。
//
// 两条 seeding 路径都必须逐比特对齐 CPython：
//   - NewByString(seed)：字符串 seeding（sha512(seed+sha512(seed)) 视为
//     big-endian 大整数 → init_by_array），对齐 CPython random.seed(str)；
//   - NewByInt(n)：整数 seeding（32 位字拆分 → init_by_array），
//     对齐 CPython random.Random(int)（不走 sha512，与字符串路径不同源）。
//
// ⚠️ 修改本包任意一行都可能改变既有基线的用例集合——改动必须附双轨对账证据
// （random_diff 基线复现 / interaction_probe 同 seed 统计一致）。
package pyrandom

import (
	"crypto/sha512"
	"math/bits"
)

// Random CPython random 实例的逐比特复刻。
type Random struct {
	mt  [624]uint32
	idx int
}

// NewByString 字符串 seeding（CPython random.seed(str)）。
func NewByString(seed string) *Random {
	sb := []byte(seed)
	h := sha512.Sum512(sb)
	buf := append(sb, h[:]...)
	// nbits：buf 视为 big-endian 大整数的有效 bit 数
	nbits := 0
	for i, b := range buf {
		if b != 0 {
			nbits = (len(buf)-1-i)*8 + bits.Len8(b)
			break
		}
	}
	keyUsed := 1
	if nbits > 0 {
		keyUsed = (nbits-1)/32 + 1
	}
	key := make([]uint32, keyUsed)
	L := len(buf)
	for i := 0; i < keyUsed; i++ {
		start := L - 4*(i+1)
		var w uint32
		for j := 0; j < 4; j++ {
			var b byte
			if start+j >= 0 {
				b = buf[start+j]
			}
			w = w<<8 | uint32(b)
		}
		key[i] = w
	}
	r := &Random{}
	r.initByArray(key)
	return r
}

// NewByInt 整数 seeding（CPython random.Random(int)，直接 init_by_array，
// 不走 sha512——与字符串 seeding 是两条路径）。
func NewByInt(n uint64) *Random {
	key := []uint32{0}
	if n > 0 {
		key = nil
		for n > 0 {
			key = append(key, uint32(n))
			n >>= 32
		}
	}
	r := &Random{}
	r.initByArray(key)
	return r
}

func (r *Random) initGenrand(s uint32) {
	r.mt[0] = s
	for i := 1; i < 624; i++ {
		r.mt[i] = 1812433253*(r.mt[i-1]^(r.mt[i-1]>>30)) + uint32(i)
	}
	r.idx = 624
}

func (r *Random) initByArray(key []uint32) {
	r.initGenrand(19650218)
	i, j := 1, 0
	k := 624
	if len(key) > k {
		k = len(key)
	}
	for ; k > 0; k-- {
		r.mt[i] = (r.mt[i] ^ (r.mt[i-1]^(r.mt[i-1]>>30))*1664525) + key[j] + uint32(j)
		i++
		j++
		if i >= 624 {
			r.mt[0] = r.mt[623]
			i = 1
		}
		if j >= len(key) {
			j = 0
		}
	}
	for k = 623; k > 0; k-- {
		r.mt[i] = (r.mt[i] ^ (r.mt[i-1]^(r.mt[i-1]>>30))*1566083941) - uint32(i)
		i++
		if i >= 624 {
			r.mt[0] = r.mt[623]
			i = 1
		}
	}
	r.mt[0] = 0x80000000
}

func (r *Random) genrandUint32() uint32 {
	if r.idx >= 624 {
		for i := 0; i < 624; i++ {
			y := r.mt[i]&0x80000000 | r.mt[(i+1)%624]&0x7fffffff
			r.mt[i] = r.mt[(i+397)%624] ^ (y >> 1)
			if y&1 != 0 {
				r.mt[i] ^= 0x9908b0df
			}
		}
		r.idx = 0
	}
	y := r.mt[r.idx]
	r.idx++
	y ^= y >> 11
	y ^= (y << 7) & 0x9d2c5680
	y ^= (y << 15) & 0xefc60000
	y ^= y >> 18
	return y
}

// Getrandbits 对齐 CPython：k ≤ 32 时一发右移。
func (r *Random) Getrandbits(k int) uint32 { return r.genrandUint32() >> (32 - k) }

// Randbelow 对齐 Python _randbelow_with_getrandbits（拒绝采样）。
func (r *Random) Randbelow(n int) int {
	k := bits.Len(uint(n))
	for {
		v := int(r.Getrandbits(k))
		if v < n {
			return v
		}
	}
}

// Randint 对齐 CPython random.randint（双闭区间）。
func (r *Random) Randint(a, b int) int { return a + r.Randbelow(b-a+1) }

// Choice 对齐 CPython random.choice（[0,n) 均匀）。
func (r *Random) Choice(n int) int { return r.Randbelow(n) }

// Random 对齐 CPython genrand_res53（53 位精度 [0,1)）。
func (r *Random) Random() float64 {
	a := uint64(r.genrandUint32() >> 5)
	b := uint64(r.genrandUint32() >> 6)
	return float64(a*67108864+b) * (1.0 / 9007199254740992.0)
}

// Choices 加权选择：对齐 CPython random.choices（accumulate + bisect_right）。
func (r *Random) Choices(weights []int) int {
	total := 0.0
	cum := make([]float64, len(weights))
	for i, w := range weights {
		total += float64(w)
		cum[i] = total
	}
	x := r.Random() * total
	// bisect_right(cum, x)：第一个 cum[i] > x 的下标
	lo, hi := 0, len(cum)
	for lo < hi {
		mid := (lo + hi) / 2
		if x < cum[mid] {
			hi = mid
		} else {
			lo = mid + 1
		}
	}
	if lo >= len(cum) {
		lo = len(cum) - 1
	}
	return lo
}

// Sample 对齐 CPython random.sample（population 视为 [1..n]）：
// n ≤ setsize(21) 用池洗牌，否则 set 拒绝采样。
func (r *Random) Sample(n, k int) []int {
	result := make([]int, k)
	setsize := 21
	if k > 5 {
		lg := 0
		for p := 1; p < k*3; p *= 4 {
			lg++
		}
		setsize += 1 << (2 * lg)
	}
	if n <= setsize {
		pool := make([]int, n)
		for i := 0; i < n; i++ {
			pool[i] = i + 1
		}
		for i := 0; i < k; i++ {
			j := r.Randbelow(n - i)
			result[i] = pool[j]
			pool[j] = pool[n-i-1]
		}
	} else {
		selected := map[int]bool{}
		for i := 0; i < k; i++ {
			j := r.Randbelow(n)
			for selected[j] {
				j = r.Randbelow(n)
			}
			result[i] = j + 1
			selected[j] = true
		}
	}
	return result
}

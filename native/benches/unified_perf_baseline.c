//! 统一模式后端性能基线用例
//!
//! 该用例通过 50 个逆序元素的冒泡排序产生约 10 万 VM 步，
//! 用于测量 `UnifiedEngine` 在 release 模式下的单步执行耗时与内存占用。
//!
//! 运行方式：
//!   cd native && cargo run --release --bin vitro_cli -- unified benches/unified_perf_baseline.c --max-steps 200000

#include <stdio.h>

void bubbleSort(int arr[], int n) {
    for (int i = 0; i < n - 1; i++) {
        for (int j = 0; j < n - i - 1; j++) {
            if (arr[j] > arr[j + 1]) {
                int temp = arr[j];
                arr[j] = arr[j + 1];
                arr[j + 1] = temp;
            }
        }
    }
}

int main() {
    int arr[50];
    int n = 50;
    for (int i = 0; i < n; i++) {
        arr[i] = n - i;
    }
    bubbleSort(arr, n);
    for (int i = 0; i < n; i++) {
        printf("%d ", arr[i]);
    }
    printf("\n");
    return 0;
}

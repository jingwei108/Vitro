// native/runtime_libc/vitro/sort_int.cpp
// Vitro 内置 int 数组排序的 C++ 实现

template<class T>
static void vitro_sort_int_swap(T* a, T* b) {
    T t = *a;
    *a = *b;
    *b = t;
}

template<class T>
static void vitro_sort_int_qsort(T* a, int left, int right) {
    if (left >= right) {
        return;
    }
    T pivot = a[(left + right) / 2];
    int i = left;
    int j = right;
    while (i <= j) {
        while (a[i] < pivot) {
            i++;
        }
        while (a[j] > pivot) {
            j--;
        }
        if (i <= j) {
            vitro_sort_int_swap(&a[i], &a[j]);
            i++;
            j--;
        }
    }
    if (left < j) {
        vitro_sort_int_qsort(a, left, j);
    }
    if (i < right) {
        vitro_sort_int_qsort(a, i, right);
    }
}

template<class T>
void vitro_sort_int(T* a, int n) {
    if (n > 1) {
        vitro_sort_int_qsort(a, 0, n - 1);
    }
}

void __vitro_force_instantiate_vitro_sort_int() {
    int a[5] = {3, 1, 4, 1, 5};
    vitro_sort_int(a, 5);
}

// native/runtime_libc/vitro/vector.cpp
// Vitro 内置容器 vector<T> 的 C++ 模板实现

template <class T>
class vitro_vec {
    int n;
    int m;
    T* a;

public:
    vitro_vec() {
        n = 0;
        m = 0;
        a = (T*)0;
    }

    void push_back(T x) {
        if (n == m) {
            m = m ? m * 2 : 2;
            T* na = new T[m];
            for (int i = 0; i < n; i++) {
                na[i] = a[i];
            }
            delete[] a;
            a = na;
        }
        a[n++] = x;
    }

    T pop_back() {
        if (n == 0) {
            return (T)0;
        }
        return a[--n];
    }

    int size() {
        return n;
    }

    int capacity() {
        return m;
    }

    T front() {
        if (n == 0) {
            return (T)0;
        }
        return a[0];
    }

    T back() {
        if (n == 0) {
            return (T)0;
        }
        return a[n - 1];
    }

    void pop_front() {
        if (n == 0) {
            return;
        }
        for (int i = 0; i < n - 1; i++) {
            a[i] = a[i + 1];
        }
        n--;
    }

    T get(int i) {
        return a[i];
    }

    void clear() {
        n = 0;
    }

    ~vitro_vec() {
        delete[] a;
    }
};

template class vitro_vec<int>;
template class vitro_vec<float>;
template class vitro_vec<char>;

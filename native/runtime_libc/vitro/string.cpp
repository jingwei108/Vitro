// native/runtime_libc/vitro/string.cpp
// Vitro 内置容器 string 的 C++ 模板实现

template <class T>
class vitro_string {
    int n;
    int m;
    T* s;

public:
    vitro_string() {
        n = 0;
        m = 0;
        s = (T*)0;
    }

    void push_back(T c) {
        if (n + 1 >= m) {
            m = m ? m * 2 : 2;
            T* ns = new T[m];
            for (int i = 0; i < n; i++) {
                ns[i] = s[i];
            }
            delete[] s;
            s = ns;
        }
        s[n++] = c;
        s[n] = (T)0;
    }

    T pop_back() {
        if (n == 0) {
            return (T)0;
        }
        T c = s[--n];
        s[n] = (T)0;
        return c;
    }

    int size() {
        return n;
    }

    int capacity() {
        return m;
    }

    T get(int i) {
        return s[i];
    }

    T* c_str() {
        return s;
    }

    T front() {
        if (n == 0) {
            return (T)0;
        }
        return s[0];
    }

    T back() {
        if (n == 0) {
            return (T)0;
        }
        return s[n - 1];
    }

    void pop_front() {
        if (n == 0) {
            return;
        }
        for (int i = 0; i < n - 1; i++) {
            s[i] = s[i + 1];
        }
        n--;
        s[n] = (T)0;
    }

    void clear() {
        n = 0;
        if (s) {
            s[0] = (T)0;
        }
    }

    ~vitro_string() {
        delete[] s;
    }
};

template class vitro_string<char>;

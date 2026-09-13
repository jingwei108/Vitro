// native/runtime_libc/vitro/list.cpp
// Vitro 内置容器 list<T> 的 C++ 模板实现

template <class T>
class vitro_list_node {
public:
    T data;
    vitro_list_node<T>* next;
};

template <class T>
class vitro_list {
    vitro_list_node<T>* head;
    vitro_list_node<T>* tail;
    int n;

public:
    vitro_list() {
        head = (vitro_list_node<T>*)0;
        tail = (vitro_list_node<T>*)0;
        n = 0;
    }

    void push_back(T x) {
        vitro_list_node<T>* node = new vitro_list_node<T>;
        node->data = x;
        node->next = (vitro_list_node<T>*)0;
        if (tail) {
            tail->next = node;
        } else {
            head = node;
        }
        tail = node;
        n++;
    }

    void push_front(T x) {
        vitro_list_node<T>* node = new vitro_list_node<T>;
        node->data = x;
        node->next = head;
        head = node;
        if (!tail) {
            tail = node;
        }
        n++;
    }

    T pop_back() {
        if (!head) {
            return (T)0;
        }
        if (head == tail) {
            T val = head->data;
            delete head;
            head = (vitro_list_node<T>*)0;
            tail = (vitro_list_node<T>*)0;
            n = 0;
            return val;
        }
        vitro_list_node<T>* p = head;
        while (p->next != tail) {
            p = p->next;
        }
        T val = tail->data;
        delete tail;
        tail = p;
        p->next = (vitro_list_node<T>*)0;
        n--;
        return val;
    }

    int size() {
        return n;
    }

    T front() {
        if (!head) {
            return (T)0;
        }
        return head->data;
    }

    T back() {
        if (!tail) {
            return (T)0;
        }
        return tail->data;
    }

    void pop_front() {
        if (!head) {
            return;
        }
        vitro_list_node<T>* node = head;
        head = node->next;
        if (!head) {
            tail = (vitro_list_node<T>*)0;
        }
        delete node;
        n--;
    }

    T get(int i) {
        vitro_list_node<T>* p = head;
        while (i-- > 0 && p != (vitro_list_node<T>*)0) {
            p = p->next;
        }
        if (p == (vitro_list_node<T>*)0) {
            return (T)0;
        }
        return p->data;
    }

    void clear() {
        vitro_list_node<T>* p = head;
        while (p != (vitro_list_node<T>*)0) {
            vitro_list_node<T>* next = p->next;
            delete p;
            p = next;
        }
        head = (vitro_list_node<T>*)0;
        tail = (vitro_list_node<T>*)0;
        n = 0;
    }

    ~vitro_list() {
        clear();
    }
};

template class vitro_list_node<int>;
template class vitro_list<int>;

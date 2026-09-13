// @category: baseline
#include <stdlib.h>
#include <stdio.h>
struct Node { int data; struct Node* next; }; struct Node* push(struct Node* top, int x) { struct Node* node = (struct Node*)malloc(sizeof(struct Node)); node->data = x; node->next = top; return node; } struct Node* pop(struct Node* top) { if (top == NULL) return NULL; struct Node* temp = top; top = top->next; free(temp); return top; } int main() { struct Node* top = NULL; top = push(top, 10); top = push(top, 20); top = push(top, 30); printf("%d", top->data); top = pop(top); printf(" %d", top->data); return 0; }

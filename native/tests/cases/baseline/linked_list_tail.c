// @category: baseline
#include <stdlib.h>
#include <stdio.h>
struct Node { int data; struct Node* next; }; struct Node* createNode(int data) { struct Node* node = (struct Node*)malloc(sizeof(struct Node)); node->data = data; node->next = NULL; return node; } struct Node* append(struct Node* head, int data) { struct Node* newNode = createNode(data); if (head == NULL) return newNode; struct Node* p = head; while (p->next != NULL) p = p->next; p->next = newNode; return head; } int main() { struct Node* head = NULL; head = append(head, 1); append(head, 2); append(head, 3); printf("%d %d %d", head->data, head->next->data, head->next->next->data); return 0; }

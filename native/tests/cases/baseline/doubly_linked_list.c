// @category: baseline
#include <stdlib.h>
#include <stdio.h>
struct DNode { int data; struct DNode* prev; struct DNode* next; }; struct DNode* createNode(int data) { struct DNode* node = (struct DNode*)malloc(sizeof(struct DNode)); node->data = data; node->prev = NULL; node->next = NULL; return node; } struct DNode* append(struct DNode* head, int data) { struct DNode* newNode = createNode(data); if (head == NULL) return newNode; struct DNode* p = head; while (p->next != NULL) p = p->next; p->next = newNode; newNode->prev = p; return head; } int main() { struct DNode* head = NULL; head = append(head, 1); head = append(head, 2); printf("%d %d", head->data, head->next->data); return 0; }

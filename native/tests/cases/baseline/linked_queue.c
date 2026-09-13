// @category: baseline
#include <stdio.h>
#include <stddef.h>
#include <stdlib.h>
struct QNode { int data; struct QNode* next; }; struct LinkedQueue { struct QNode* front; struct QNode* rear; }; void init(struct LinkedQueue* q) { q->front = NULL; q->rear = NULL; } void enqueue(struct LinkedQueue* q, int x) { struct QNode* node = (struct QNode*)malloc(sizeof(struct QNode)); node->data = x; node->next = NULL; if (q->rear == NULL) { q->front = node; q->rear = node; } else { q->rear->next = node; q->rear = node; } } int dequeue(struct LinkedQueue* q) { if (q->front == NULL) return -1; struct QNode* temp = q->front; int x = temp->data; q->front = q->front->next; if (q->front == NULL) q->rear = NULL; free(temp); return x; } int main() { struct LinkedQueue q; init(&q); enqueue(&q, 10); enqueue(&q, 20); enqueue(&q, 30); printf("%d %d %d", dequeue(&q), dequeue(&q), dequeue(&q)); return 0; }

typedef char* va_list;

void __vitro_va_start(char** ap, void* last, int last_size);
void* __vitro_va_arg(char** ap, int size);
void __vitro_va_end(char** ap);
void __vitro_va_copy(char** dst, char** src);

#define va_start(ap, last) __vitro_va_start(&(ap), &(last), sizeof(last))
#define va_arg(ap, type) (*(type*)__vitro_va_arg(&(ap), sizeof(type)))
#define va_end(ap) __vitro_va_end(&(ap))
#define va_copy(dst, src) __vitro_va_copy(&(dst), &(src))

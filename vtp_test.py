import socket
import struct
import sys

def main():
    host = '127.0.0.1'
    port = 8080

    # Пробуем подключиться к терминалу
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.connect((host, port))
        print(f"[*] Успешное подключение к VoidGrid на {host}:{port}")
    except ConnectionRefusedError:
        print("[!] Ошибка: Сервер VoidGrid не запущен или порт занят.")
        sys.exit(1)

    def send_vtp(payload):
        s.sendall(payload)

    # --- ИНИЦИАЛИЗАЦИЯ ПЕЧАТНОЙ МАШИНКИ ---
    init_payload = bytearray()
    
    # 1. SET_BUFFER ("main_buf")
    init_payload.append(0x01)
    init_payload.extend(struct.pack('<H', 8))
    init_payload.extend(b"main_buf")

    # 2. SET_FG_COLOR (Сделаем классический терминальный зеленый)
    init_payload.extend([0x03, 0, 255, 0, 255])

    # 3. SET_CURSOR (x=1, y=1)
    current_x = 4
    current_y = 13
    init_payload.append(0x02)
    init_payload.extend(struct.pack('<H', current_x))
    init_payload.extend(struct.pack('<H', current_y))

    # Отправляем настройки
    send_vtp(init_payload)

    print("\n[ РЕЖИМ ПЕЧАТНОЙ МАШИНКИ АКТИВЕН ]")
    print("Вводи текст и нажимай Enter. Для выхода нажми Ctrl+C.\n")

    # --- ГЛАВНЫЙ ЦИКЛ ВВОДА ---
    try:
        while True:
            # Читаем строку из консоли Питона
            raw_text = input(">> ")
            
            # Делаем ЗАГЛАВНЫМИ
            text = raw_text.upper()
            
            payload = bytearray()
            
            # Если текст не пустой, отправляем PRINT_STRING
            if text:
                encoded_text = text.encode('utf-8')
                payload.append(0x11) # Opcode: PRINT_STRING
                payload.extend(struct.pack('<H', len(encoded_text)))
                payload.extend(encoded_text)
            
            # Обрабатываем Enter: сдвигаем Y на 1 вниз, X возвращаем на 1
            current_y += 1
            payload.append(0x02) # Opcode: SET_CURSOR
            payload.extend(struct.pack('<H', current_x))
            payload.extend(struct.pack('<H', current_y))
            
            # Отправляем пакет по сети
            send_vtp(payload)

    except KeyboardInterrupt:
        print("\n[*] Отключение от терминала...")
        s.close()
        sys.exit(0)

if __name__ == "__main__":
    main()
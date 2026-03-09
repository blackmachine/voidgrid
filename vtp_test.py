import socket
import struct
import sys
import time

# --- НАСТРОЙКИ ЦВЕТОВ ---
BASE_COLOR = (0, 255, 127, 255)      # Добавили альфа-канал (255)
HOT_COLOR = (200, 255, 230, 255)     # Добавили альфа-канал (255)

def interpolate_color(c1, c2, factor):
    """Смешивает два цвета. factor=0.0 -> c1, factor=1.0 -> c2"""
    r = int(c1[0] + (c2[0] - c1[0]) * factor)
    g = int(c1[1] + (c2[1] - c1[1]) * factor)
    b = int(c1[2] + (c2[2] - c1[2]) * factor)
    return (r, g, b, 255)

def get_fade_color(age):
    """Возвращает цвет в зависимости от того, сколько 'тиков' назад напечатан символ"""
    if age == 0:
        return HOT_COLOR
    elif age == 1:
        return interpolate_color(HOT_COLOR, BASE_COLOR, 0.4)
    elif age == 2:
        return interpolate_color(HOT_COLOR, BASE_COLOR, 0.8)
    else:
        return BASE_COLOR

def main():
    host = '127.0.0.1'
    port = 8080

    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.connect((host, port))
        print(f"[*] Успешное подключение к VoidGrid на {host}:{port}")
    except ConnectionRefusedError:
        print("[!] Ошибка: Сервер VoidGrid не запущен или порт занят.")
        sys.exit(1)

    def send_vtp(payload):
        s.sendall(payload)

    # --- ИНИЦИАЛИЗАЦИЯ ---
    init_payload = bytearray()
    init_payload.append(0x01) # SET_BUFFER
    init_payload.extend(struct.pack('<H', 8))
    init_payload.extend(b"main_buf")
    send_vtp(init_payload)

    current_x = 4
    current_y = 13

    print("\n[ КИНЕМАТОГРАФИЧНАЯ ПЕЧАТНАЯ МАШИНКА ]")
    print("Вводи текст (латиницей) и нажимай Enter.\n")

    try:
        while True:
            raw_text = input(">> ")
            text = raw_text.upper()
            
            if not text:
                continue

            length = len(text)
            
            # Анимируем печать (кол-во шагов = длина текста + хвост для затухания)
            for step in range(length + 3):
                payload = bytearray()
                
                # На каждом шаге возвращаем курсор в начало текущей строки!
                payload.append(0x02) # SET_CURSOR
                payload.extend(struct.pack('<H', current_x))
                payload.extend(struct.pack('<H', current_y))
                
                # Печатаем все символы от 0 до текущего шага
                chars_to_print = min(step + 1, length)
                for j in range(chars_to_print):
                    age = step - j # Насколько символ "старый"
                    r, g, b, a = get_fade_color(age)
                    
                    # 1. Меняем цвет перед печатью символа
                    payload.append(0x03) # SET_FG_COLOR
                    payload.extend([r, g, b, a])
                    
                    # 2. Печатаем 1 символ (PRINT_CHAR сам сдвинет курсор на +1 по X)
                    # Формат PRINT_CHAR требует u32 (4 байта) под код символа
                    payload.append(0x10) 
                    char_code = ord(text[j])
                    payload.extend(struct.pack('<I', char_code))
                    
                # Отправляем сформированный кадр в движок
                send_vtp(payload)
                
                # Задержка между ударами по клавишам (40 мс)
                time.sleep(0.04) 
            
            # После завершения анимации строки переходим на новую
            current_y += 1

    except KeyboardInterrupt:
        print("\n[*] Отключение...")
        s.close()
        sys.exit(0)

if __name__ == "__main__":
    main()
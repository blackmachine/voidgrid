fn init() {
    this.x = 4.0;
    this.y = 15;
    this.speed = 0.5;
    this.dir = 1.0;
}

fn update(events) {
    
    // Получаем массив размеров и достаем значения по индексам
    let bw = get_buffer_width("main_buf");
    let bh = get_buffer_height("main_buf");
    
  

    // --- ОТРИСОВКА ---
    
    set_buffer("main_buf");
    
    set_fg(255, 200, 50, 255);
    set_cursor(4, 19);
    write_text(`${bw} X ${bh}`);


    set_buffer("tx_buf");
    set_fg(64, 48, 32, 255);
    set_bg(192, 192, 255, 255);
    set_cursor(10, 1);
    write_text(`Oh hi Mark`, `bold`);
    set_cursor(10, 2);
    write_text(`It's you`, `thin`);
    set_bg(0, 0, 0, 0);


    for e in events {
    if e.type == "MousePress" {
        let cell = mouse_to_cell("tx_buf", e.x, e.y);
        if cell != () {
            set_fg(255, 64, 0, 255);
            set_cursor(cell.x, cell.y);
            write_text(`+`);
            log(`${cell.x} - ${cell.y}`);
        }
    }
}
}